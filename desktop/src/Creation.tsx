import { useEffect, useRef, useState } from 'react';
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import Markdown from 'react-markdown';
import { Choice } from './Choice';
import { StudioEntrance, StudioFormat, StudioLaunch } from './navigation';
import { AssetEntry, AssetLibrary, DELIVERABLE_LABELS, emptyIp, loadLibrary } from './library';

interface AdapterStatus {
  id: string;
  displayName: string;
  available: boolean;
  capabilities: string[];
  detail?: string | null;
}

interface Message { id: string; role: string; text: string }

interface ArtifactVersion {
  version: number;
  path: string;
  taskId: string;
  note: string;
  createdAt: number;
}

interface BlueprintSlot {
  slotId: string;
  label: string;
  text: string;
  visualPrompt: string;
  durationSec?: number | null;
  status: string;
  error?: string | null;
  versions: ArtifactVersion[];
}

interface Blueprint {
  summaryCopy: string;
  bodyMarkdown: string;
  slots: BlueprintSlot[];
}

interface CreationJob {
  parentId?: string | null;
  sourceTaskId?: string | null;
  slotId?: string | null;
  id: string;
  title: string;
  deliverable: StudioFormat;
  adapterId: string;
  includeIp: boolean;
  track: string;
  skillId: string;
  skillName: string;
  promptIds: string[];
  snapshotSeq: number;
  stage: 'drafting' | 'blueprintReady' | 'rendering' | 'review';
  blueprintApproved: boolean;
  blueprint: Blueprint;
  status: string;
  messages: Message[];
  images: string[];
  error?: string | null;
  turnId?: string | null;
  activity: string;
}

const native = isTauri();
const isJobBusy = (j?: CreationJob) => !!j && (['starting', 'running'].includes(j.status) || j.blueprint.slots.some(s => s.status === 'running'));
const errText = (e: unknown) => e instanceof Error ? e.message : String(e);

function defaultSkillForFormat(format: StudioFormat): string {
  return format === 'article' ? 'builtin:article-illustrated' : format === 'video' ? 'builtin:video-storyboard' : 'builtin:cards-series';
}

export function Creation({ name, launch, back }: { name: StudioEntrance; launch?: StudioLaunch; back: () => void }) {
  const initialFormat: StudioFormat = name === '视频' ? 'video' : (launch?.deliverable && launch.deliverable !== 'video' ? launch.deliverable : 'cards');
  const [format, setFormat] = useState<StudioFormat>(initialFormat);
  const [builtins, setBuiltins] = useState<AssetEntry[]>([]);
  const [library, setLibrary] = useState<AssetLibrary>({ ip: emptyIp(), entries: [], imageDirectory: '', categories: [], tracks: [] });
  const [adapters, setAdapters] = useState<AdapterStatus[]>([]);
  const [adapterId, setAdapterId] = useState('codex');
  const [jobs, setJobs] = useState<CreationJob[]>([]);
  const [selectedId, setSelectedId] = useState(() => launch ? '' : (localStorage.getItem(`creatorx.studio.${initialFormat}`) || ''));
  const [includeIp, setIncludeIp] = useState(true);
  const [track, setTrack] = useState(launch?.track || '');
  const [skillId, setSkillId] = useState(launch?.skillId ?? defaultSkillForFormat(initialFormat));
  const [promptIds, setPromptIds] = useState<string[]>(launch?.promptId ? [launch.promptId] : []);
  const [attachmentsDirty, setAttachmentsDirty] = useState(false);
  const [contextOpen, setContextOpen] = useState(false);
  const [text, setText] = useState('');
  const [directory, setDirectory] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [enlarged, setEnlarged] = useState<string>();
  const [editingBlueprint, setEditingBlueprint] = useState(false);
  const [bpDraft, setBpDraft] = useState<Blueprint>({ summaryCopy: '', bodyMarkdown: '', slots: [] });
  const [articleView, setArticleView] = useState<'preview' | 'markdown'>('preview');
  const [slotVersions, setSlotVersions] = useState<Record<string, number>>({});
  const [revisingSlot, setRevisingSlot] = useState<string>('');
  const [revisionNote, setRevisionNote] = useState('');
  const [promoting, setPromoting] = useState(false);
  const [promoteName, setPromoteName] = useState('');
  const [promoteCategory, setPromoteCategory] = useState('视觉画风');
  const [promotePrompt, setPromotePrompt] = useState('');
  const [copiedKey, setCopiedKey] = useState('');

  const zoomRef = useRef<HTMLDialogElement>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const followRef = useRef(true);
  const hydratedJobRef = useRef('');
  const handledLaunchRef = useRef<number | undefined>(undefined);
  const jobsRef = useRef(jobs); jobsRef.current = jobs;
  const submittingRef = useRef(submitting); submittingRef.current = submitting;

  const mainJobs = jobs.filter(j => !j.parentId && j.deliverable === format);
  const job = mainJobs.find(j => j.id === selectedId);
  const allSkills = [...builtins, ...library.entries.filter(e => e.kind === 'skill' && e.maturity !== 'draft')];
  const matchingSkills = allSkills.filter(s => s.deliverable === format || s.deliverable === 'general');
  const otherSkills = allSkills.filter(s => s.deliverable !== format && s.deliverable !== 'general');
  const stylePrompts = library.entries.filter(e => e.kind === 'prompt' && e.maturity !== 'draft');

  async function refreshJobs() {
    if (!native) return [];
    const list = await invoke<CreationJob[]>('creation_list');
    setJobs(list);
    jobsRef.current = list;
    return list;
  }

  async function probeAdapters(verifyId?: string) {
    if (!native) return;
    try {
      const statuses = await invoke<AdapterStatus[]>('adapter_list', { verifyId: verifyId ?? null });
      setAdapters(statuses);
    } catch (e) { setError(errText(e)); }
  }

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const syncMaterials = () => {
      void loadLibrary().then(lib => { if (!disposed) setLibrary(lib); }).catch(e => { if (!disposed) setError(errText(e)); });
    };
    void invoke<AssetEntry[]>('creation_builtin_skills').then(b => { if (!disposed) setBuiltins(b); }).catch(e => setError(errText(e)));
    syncMaterials();
    void probeAdapters('codex');
    void refreshJobs().catch(e => setError(errText(e)));
    void invoke<string>('creation_directory').then(d => { if (!disposed) setDirectory(d); }).catch(e => setError(errText(e)));
    const onFocus = () => { syncMaterials(); void refreshJobs().catch(e => setError(errText(e))); };
    window.addEventListener('focus', onFocus);
    const timer = setInterval(() => {
      if (!disposed && document.visibilityState === 'visible' && (submittingRef.current || jobsRef.current.some(isJobBusy))) {
        void refreshJobs().catch(e => setError(errText(e)));
      }
    }, 900);
    return () => { disposed = true; clearInterval(timer); window.removeEventListener('focus', onFocus); };
  }, []);

  useEffect(() => {
    if (launch?.nonce && handledLaunchRef.current !== launch.nonce) {
      handledLaunchRef.current = launch.nonce;
      const targetFormat: StudioFormat = name === '视频' ? 'video' : (launch.deliverable && launch.deliverable !== 'video' ? launch.deliverable : 'cards');
      setFormat(targetFormat);
      hydratedJobRef.current = '';
      setSelectedId('');
      setIncludeIp(true);
      setTrack(launch.track || '');
      setSkillId(launch.skillId ?? defaultSkillForFormat(targetFormat));
      setPromptIds(launch.promptId ? [launch.promptId] : []);
      setAttachmentsDirty(false);
      setEditingBlueprint(false);
      setPromoting(false);
      composerRef.current?.focus();
    }
  }, [launch?.nonce, name]);

  useEffect(() => {
    if (!launch) localStorage.setItem(`creatorx.studio.${format}`, selectedId);
  }, [selectedId, format, launch]);

  useEffect(() => {
    if (job && hydratedJobRef.current !== job.id) {
      hydratedJobRef.current = job.id;
      setAdapterId(job.adapterId || 'codex');
      setIncludeIp(job.includeIp);
      setTrack(job.track || '');
      setSkillId(job.skillId || '');
      setPromptIds(job.promptIds || []);
      setAttachmentsDirty(false);
      setEditingBlueprint(false);
      setPromoting(false);
    }
  }, [job?.id]);

  useEffect(() => { if (enlarged) zoomRef.current?.showModal(); else zoomRef.current?.close(); }, [enlarged]);
  const lastMsg = job?.messages.at(-1)?.text;
  useEffect(() => {
    const el = messagesRef.current;
    if (el && followRef.current) el.scrollTop = el.scrollHeight;
  }, [lastMsg, selectedId]);

  function switchFormat(next: StudioFormat) {
    if (next === format) return;
    setFormat(next);
    const saved = localStorage.getItem(`creatorx.studio.${next}`) || '';
    hydratedJobRef.current = '';
    setSelectedId(saved);
    if (!saved) {
      setSkillId(defaultSkillForFormat(next));
      setPromptIds([]);
      setAttachmentsDirty(false);
    }
    setEditingBlueprint(false);
    setPromoting(false);
    setError('');
  }

  function selectJob(target?: CreationJob) {
    hydratedJobRef.current = target?.id || '';
    setSelectedId(target?.id || '');
    setAdapterId(target?.adapterId || 'codex');
    setIncludeIp(target ? target.includeIp : true);
    setTrack(target?.track || '');
    setSkillId(target ? target.skillId : defaultSkillForFormat(format));
    setPromptIds(target?.promptIds || []);
    setAttachmentsDirty(false);
    setContextOpen(false);
    setEditingBlueprint(false);
    setPromoting(false);
    setText('');
    setError('');
    followRef.current = true;
    composerRef.current?.focus();
  }

  async function send() {
    const message = text.trim();
    if (!message || submitting || (job && ['starting', 'running'].includes(job.status))) return;
    setSubmitting(true); setError(''); followRef.current = true;
    const prevIds = new Set(jobs.map(j => j.id));
    try {
      const result = job
        ? await invoke<CreationJob>('creation_continue', {
            id: job.id,
            text: message,
            attachments: attachmentsDirty ? { includeIp, track, skillId, promptIds } : null,
            adapterId,
          })
        : await invoke<CreationJob>('creation_start', {
            deliverable: format,
            adapterId,
            includeIp,
            track,
            skillId: skillId || null,
            promptIds,
            brief: message,
          });
      setSelectedId(result.id);
      setAttachmentsDirty(false);
      setContextOpen(false);
      setText(cur => cur.trim() === message ? '' : cur);
      await refreshJobs();
    } catch (e) {
      setError(errText(e));
      const latest = await refreshJobs().catch(() => []);
      if (!job) {
        const created = latest.find(j => !prevIds.has(j.id) && j.deliverable === format && !j.parentId);
        if (created) setSelectedId(created.id);
      }
    } finally {
      setSubmitting(false);
      composerRef.current?.focus();
    }
  }

  function openBlueprintEditor() {
    const base = job?.blueprint || { summaryCopy: '', bodyMarkdown: '', slots: [] };
    const defaultPrefix = format === 'article' ? 'fig' : format === 'video' ? 'shot' : 'card';
    const slots = base.slots.length ? base.slots.map(s => ({ ...s })) : [{
      slotId: `${defaultPrefix}-1`,
      label: format === 'article' ? '封面图' : format === 'video' ? '镜头 1 · 开场钩子' : '第 1 页 · 封面',
      text: '',
      visualPrompt: '',
      durationSec: format === 'video' ? 4 : null,
      status: 'pending',
      versions: [],
    }];
    setBpDraft({ summaryCopy: base.summaryCopy, bodyMarkdown: base.bodyMarkdown, slots });
    setEditingBlueprint(true);
  }

  async function saveBlueprintDraft(approve: boolean) {
    if (!job) return;
    setSubmitting(true); setError('');
    try {
      await invoke<CreationJob>('creation_save_blueprint', { id: job.id, blueprint: bpDraft, approve });
      setEditingBlueprint(false);
      await refreshJobs();
    } catch (e) { setError(errText(e)); }
    finally { setSubmitting(false); }
  }

  async function approveCurrentBlueprint() {
    if (!job) return;
    setSubmitting(true); setError('');
    try {
      await invoke<CreationJob>('creation_save_blueprint', { id: job.id, blueprint: job.blueprint, approve: true });
      await refreshJobs();
    } catch (e) { setError(errText(e)); }
    finally { setSubmitting(false); }
  }

  async function renderSlot(slotId: string, note = '') {
    if (!job) return;
    setError('');
    try {
      await invoke<CreationJob>('creation_render_slot', { id: job.id, slotId, revisionNote: note || null });
      setRevisingSlot('');
      setRevisionNote('');
      await refreshJobs();
    } catch (e) { setError(errText(e)); }
  }

  async function renderPendingSlots() {
    if (!job) return;
    const pending = job.blueprint.slots.filter(s => s.versions.length === 0 && s.status !== 'running').slice(0, 3);
    for (const s of pending) {
      await renderSlot(s.slotId);
    }
  }

  async function submitPromoteShowcase() {
    if (!job || !promoteName.trim() || !promotePrompt.trim()) return;
    setSubmitting(true); setError('');
    try {
      await invoke('creation_promote_showcase', {
        id: job.id,
        request: {
          name: promoteName.trim(),
          category: promoteCategory,
          prompt: promotePrompt.trim(),
          sampleInput: job.title,
          sampleOutput: job.blueprint.summaryCopy || job.blueprint.bodyMarkdown.slice(0, 600),
          notes: `提炼自 ${DELIVERABLE_LABELS[job.deliverable]} 任务`,
          slotIds: [],
        },
      });
      setPromoting(false);
      setCopiedKey('promoted');
      setTimeout(() => setCopiedKey(''), 2500);
    } catch (e) { setError(errText(e)); }
    finally { setSubmitting(false); }
  }

  async function copyText(key: string, content: string) {
    try {
      await navigator.clipboard.writeText(content);
      setCopiedKey(key);
      setTimeout(() => setCopiedKey(''), 1800);
    } catch { setError('复制失败，请手动选中复制'); }
  }

  const artifactUrl = (ownerId: string, rel: string) => convertFileSrc(`${directory}/${ownerId}/${rel}`);
  const mainRunning = !!job && ['starting', 'running'].includes(job.status);
  const locked = submitting || mainRunning;
  const activeAdapter = adapters.find(a => a.id === adapterId);

  function selectedVersionForSlot(slot: BlueprintSlot): ArtifactVersion | undefined {
    if (!slot.versions.length) return undefined;
    const chosen = slotVersions[slot.slotId];
    return slot.versions.find(v => v.version === chosen) || slot.versions[slot.versions.length - 1];
  }

  async function openSandboxFile(path?: string) {
    if (!path || !job) return;
    try { await invoke('creation_open_file', { id: job.id, path }); }
    catch (e) { setError(errText(e)); }
  }

  const stageLabel = !job
    ? '1 立意与样板装配'
    : job.stage === 'review'
      ? '3 成品检验与版本微调'
      : job.blueprintApproved
        ? '2 蓝图已放行 · 可按槽位生成'
        : (job.blueprint.slots.length > 0 || job.blueprint.bodyMarkdown.trim())
          ? '2 蓝图待确认（门禁保护中）'
          : '1 立意与内容打磨';

  return (
    <section className="page chat-page" aria-label={`${name}创作台`}>
      <header className="page-header chat-header">
        <button className="icon-button" aria-label="返回上一页" onClick={back}>‹</button>
        {name === '图文' ? (
          <div className="segmented-pills" role="group" aria-label="图文交付制式">
            <button className={format === 'cards' ? 'active' : ''} onClick={() => switchFormat('cards')}>卡片图文</button>
            <button className={format === 'article' ? 'active' : ''} onClick={() => switchFormat('article')}>长文配图</button>
          </div>
        ) : (
          <span className="studio-mode-pill">视频创作</span>
        )}
        <Choice
          className="chat-session-select"
          label="切换创作任务"
          value={job?.id || ''}
          options={[{ value: '', label: '新创作任务' }, ...[...mainJobs].reverse().map(j => ({ value: j.id, label: j.title }))]}
          onChange={id => selectJob(mainJobs.find(j => j.id === id))}
        />
        <span className="header-space" />
        <Choice
          className="adapter-select"
          label="切换本地 Agent 引擎"
          value={adapterId}
          options={(adapters.length ? adapters : [{ id: 'codex', displayName: 'Codex · 6.1 Sol · 中等', available: true, capabilities: [] }]).map(a => ({
            value: a.id,
            label: a.displayName,
          }))}
          onChange={next => {
            setAdapterId(next);
            void probeAdapters(next);
          }}
        />
        <button className="text-button codex-connection" onClick={() => { void probeAdapters(adapterId); }}>
          {activeAdapter ? (activeAdapter.available ? '引擎就绪' : '检查连接') : '连接中…'}
        </button>
        <button className="icon-button" aria-label="新建创作任务" onClick={() => selectJob()} disabled={submitting}>＋</button>
      </header>

      <div className="chat-layout studio-layout">
        {/* Left: Dialogue & Context Snapshot Controls */}
        <div className="chat-main">
          <div
            className="chat-messages"
            ref={messagesRef}
            onScroll={e => {
              const view = e.currentTarget;
              followRef.current = view.scrollHeight - view.scrollTop - view.clientHeight < 100;
            }}
          >
            {!job && (
              <div className="chat-empty studio-welcome">
                <p>
                  {format === 'cards'
                    ? '小红书／抖音卡片图文：先聊选题，确认每页卡片蓝图后再出图'
                    : format === 'article'
                      ? '微信公众号长文配图：先磨好文章结构与插图位，再配图成稿'
                      : '抖音／小红书视频：先定黄金钩子与口播稿，再拆解镜头分镜'}
                </p>
                {activeAdapter?.detail && !activeAdapter.available && (
                  <small className="adapter-hint">{activeAdapter.detail}</small>
                )}
              </div>
            )}
            {job?.messages.map(m => (
              <article key={m.id} className={`chat-message ${m.role}`} aria-label={m.role === 'user' ? '你的消息' : 'Agent 回复'}>
                {m.role === 'user' ? (
                  <p>{m.text}</p>
                ) : (
                  <Markdown
                    skipHtml
                    components={{
                      a: ({ href, children }) => href && /^https?:\/\//.test(href)
                        ? <a href={href} target="_blank" rel="noreferrer">{children}</a>
                        : <button className="inline-file-link" onClick={() => { void openSandboxFile(href); }}>{children}</button>,
                      img: () => null,
                    }}
                  >
                    {m.text}
                  </Markdown>
                )}
              </article>
            ))}
            {mainRunning && <p className="chat-activity" role="status">{job?.activity || '正在思考与整理蓝图…'}</p>}
          </div>

          <div className="chat-composer-area">
            {(error || job?.error) && <p className="inline-error" role="alert">{error || job?.error}</p>}
            {copiedKey === 'promoted' && <p className="inline-notice" role="status">已将当前作品样板提炼保存至样板间</p>}
            <div className="chat-composer" data-editing={(text.trim() || editingBlueprint || promoting) ? '' : undefined}>
              {contextOpen && (
                <div className="chat-materials">
                  <div className="context-row">
                    <label className="ip-toggle">
                      <input
                        type="checkbox"
                        checked={includeIp}
                        disabled={locked}
                        onChange={e => { setIncludeIp(e.target.checked); setAttachmentsDirty(true); }}
                      />
                      <span>带入 IP 底座{library.ip.name ? `（${library.ip.name}）` : '（未设，可在样板间配置）'}</span>
                    </label>
                    <Choice
                      className="composer-track"
                      label="本次赛道"
                      value={track}
                      disabled={locked}
                      options={[
                        { value: '', label: '通用赛道' },
                        ...library.tracks.map(t => ({ value: t, label: t })),
                      ]}
                      onChange={t => { setTrack(t); setAttachmentsDirty(true); }}
                    />
                  </div>
                  <details open={stylePrompts.length > 0}>
                    <summary>挂载风格提示词样板{promptIds.length ? ` · 已选 ${promptIds.length}` : ''}</summary>
                    <div>
                      {stylePrompts.length === 0 ? (
                        <p className="sidebar-note">样板间暂无风格提示词，可直接对话或稍后在样板间积累。</p>
                      ) : stylePrompts.map(p => (
                        <label key={p.id}>
                          <input
                            type="checkbox"
                            checked={promptIds.includes(p.id)}
                            disabled={locked || (!promptIds.includes(p.id) && promptIds.length >= 16)}
                            onChange={e => {
                              setPromptIds(old => e.target.checked ? [...old, p.id] : old.filter(id => id !== p.id));
                              setAttachmentsDirty(true);
                            }}
                          />
                          <span>{p.name}</span>
                          {p.category && <small className="badge">{p.category}</small>}
                        </label>
                      ))}
                    </div>
                  </details>
                </div>
              )}
              <textarea
                ref={composerRef}
                autoFocus
                aria-label="输入创作意图或修改反馈"
                value={text}
                rows={3}
                onChange={e => setText(e.target.value)}
                onKeyDown={e => {
                  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter' && !e.nativeEvent.isComposing) {
                    e.preventDefault();
                    void send();
                  }
                }}
                placeholder={job ? '补充细节、调整蓝图，或说明需要修改的地方…' : '说说这期想分享什么主题、核心观点或真实经历…'}
              />
              <div className="chat-composer-toolbar">
                <button
                  className={`text-button context-btn ${contextOpen ? 'active' : ''}`}
                  aria-label="配置上下文快照"
                  aria-expanded={contextOpen}
                  onClick={() => setContextOpen(!contextOpen)}
                >
                  ＋ 上下文
                </button>
                <Choice
                  className="composer-skill"
                  label="选择制式样板 Skill"
                  value={skillId}
                  disabled={locked}
                  options={[
                    { value: '', label: '不挂载 Skill' },
                    ...matchingSkills.map(s => ({ value: s.id, label: s.name, group: `适配${DELIVERABLE_LABELS[format]}` })),
                    ...otherSkills.map(s => ({ value: s.id, label: s.name, group: '其他样板 Skills' })),
                  ]}
                  onChange={id => { setSkillId(id); setAttachmentsDirty(true); }}
                />
                <span className="chat-attached">
                  {[
                    includeIp && library.ip.name ? `IP:${library.ip.name}` : '',
                    track || '',
                    promptIds.length ? `${promptIds.length} 风格` : '',
                    job ? `快照#${job.snapshotSeq}` : '',
                  ].filter(Boolean).join(' · ')}
                </span>
                <span className="header-space" />
                {mainRunning ? (
                  <button
                    className="chat-send stop"
                    aria-label="停止执行"
                    disabled={submitting || !job?.turnId}
                    onClick={() => {
                      setSubmitting(true);
                      void invoke('creation_cancel', { id: job?.id }).then(refreshJobs).catch(e => setError(errText(e))).finally(() => setSubmitting(false));
                    }}
                  >
                    ■
                  </button>
                ) : (
                  <button className="chat-send" aria-label="发送" disabled={locked || !text.trim()} onClick={() => { void send(); }}>
                    {submitting ? '…' : '↑'}
                  </button>
                )}
              </div>
            </div>
          </div>
        </div>

        {/* Right: Structured Harness Blueprint & Multi-Version Deliverable Canvas */}
        <aside className="creation-results harness-canvas" aria-label="结构化蓝图与作品画布">
          <div className="harness-gate-bar">
            <span className={`gate-pill ${job?.blueprintApproved ? 'approved' : ''}`}>{stageLabel}</span>
            <span className="header-space" />
            {job && (
              <div className="gate-actions">
                <button className="text-button" onClick={() => editingBlueprint ? setEditingBlueprint(false) : openBlueprintEditor()}>
                  {editingBlueprint ? '收起编辑' : '编辑蓝图'}
                </button>
                {!job.blueprintApproved && (job.blueprint.slots.length > 0 || job.blueprint.bodyMarkdown.trim()) && (
                  <button className="text-button primary-action" disabled={submitting} onClick={() => { void approveCurrentBlueprint(); }}>
                    ✓ 确认蓝图
                  </button>
                )}
                {job.blueprint.slots.some(s => s.versions.length === 0 && s.status !== 'running') && (
                  <button className="text-button primary-action" onClick={() => { void renderPendingSlots(); }}>
                    {format === 'video' ? '生成关键帧' : '生成待出图项'}
                  </button>
                )}
                {(job.blueprint.slots.some(s => s.versions.length > 0) || job.blueprint.bodyMarkdown.trim()) && (
                  <button
                    className="text-button"
                    onClick={() => {
                      setPromoteName(job.title);
                      setPromoteCategory(format === 'video' ? '脚本钩子' : format === 'article' ? '写作母版' : '视觉画风');
                      setPromotePrompt(job.blueprint.slots[0]?.visualPrompt || job.blueprint.summaryCopy || job.title);
                      setPromoting(!promoting);
                    }}
                  >
                    ✦ 提炼样板
                  </button>
                )}
              </div>
            )}
          </div>

          {promoting && job && (
            <div className="promote-box">
              <h3>提炼为已验证样板（自动附带本任务最新成品图与输出样例）</h3>
              <div className="editor-two-grid">
                <input value={promoteName} maxLength={80} onChange={e => setPromoteName(e.target.value)} placeholder="样板名称" />
                <Choice
                  label="样板分类"
                  editable
                  value={promoteCategory}
                  options={['视觉画风', '写作母版', '脚本钩子'].map(c => ({ value: c, label: c }))}
                  onChange={setPromoteCategory}
                />
              </div>
              <textarea
                className="instructions-editor compact-textarea"
                value={promotePrompt}
                onChange={e => setPromotePrompt(e.target.value)}
                placeholder="总结这条可复用的画风／行文／分镜 Prompt"
              />
              <div className="promote-actions">
                <button className="text-button" onClick={() => setPromoting(false)}>取消</button>
                <button className="text-button primary-action" disabled={submitting || !promoteName.trim() || !promotePrompt.trim()} onClick={() => { void submitPromoteShowcase(); }}>
                  保存到样板间
                </button>
              </div>
            </div>
          )}

          {editingBlueprint && job ? (
            <BlueprintEditorPanel
              format={format}
              draft={bpDraft}
              setDraft={setBpDraft}
              busy={submitting}
              onSave={approve => { void saveBlueprintDraft(approve); }}
            />
          ) : !job || (!job.blueprint.slots.length && !job.blueprint.bodyMarkdown.trim() && !job.blueprint.summaryCopy.trim()) ? (
            <div className="results-empty">
              <span aria-hidden="true">◇</span>
              <p>
                {format === 'cards'
                  ? '在左侧与 Agent 敲定内容后，这里会呈现分页卡片蓝图与各版成图'
                  : format === 'article'
                    ? '定稿后这里会呈现排版长文、封面与插图槽位'
                    : '定稿后这里会呈现口播逐字稿与分镜镜头卡'}
              </p>
              {job && <button className="text-button" onClick={openBlueprintEditor}>手动新建蓝图槽位</button>}
            </div>
          ) : format === 'article' ? (
            <ArticleCanvas
              job={job}
              view={articleView}
              setView={setArticleView}
              copiedKey={copiedKey}
              onCopy={copyText}
              selectedVersionForSlot={selectedVersionForSlot}
              setSlotVersions={setSlotVersions}
              artifactUrl={artifactUrl}
              setEnlarged={setEnlarged}
              revisingSlot={revisingSlot}
              setRevisingSlot={setRevisingSlot}
              revisionNote={revisionNote}
              setRevisionNote={setRevisionNote}
              onRenderSlot={renderSlot}
            />
          ) : format === 'video' ? (
            <VideoCanvas
              job={job}
              copiedKey={copiedKey}
              onCopy={copyText}
              selectedVersionForSlot={selectedVersionForSlot}
              setSlotVersions={setSlotVersions}
              artifactUrl={artifactUrl}
              setEnlarged={setEnlarged}
              revisingSlot={revisingSlot}
              setRevisingSlot={setRevisingSlot}
              revisionNote={revisionNote}
              setRevisionNote={setRevisionNote}
              onRenderSlot={renderSlot}
            />
          ) : (
            <CardsCanvas
              job={job}
              copiedKey={copiedKey}
              onCopy={copyText}
              selectedVersionForSlot={selectedVersionForSlot}
              setSlotVersions={setSlotVersions}
              artifactUrl={artifactUrl}
              setEnlarged={setEnlarged}
              revisingSlot={revisingSlot}
              setRevisingSlot={setRevisingSlot}
              revisionNote={revisionNote}
              setRevisionNote={setRevisionNote}
              onRenderSlot={renderSlot}
            />
          )}
        </aside>
      </div>

      <dialog ref={zoomRef} className="image-lightbox" aria-label="查看作品原图" onCancel={e => { e.preventDefault(); setEnlarged(undefined); }} onClick={e => { if (e.target === e.currentTarget) setEnlarged(undefined); }}>
        <button className="lightbox-close icon-button" aria-label="关闭原图" onClick={() => setEnlarged(undefined)}>×</button>
        {enlarged && <img src={enlarged} alt="作品原图" />}
      </dialog>
    </section>
  );
}

function BlueprintEditorPanel({
  format, draft, setDraft, busy, onSave,
}: {
  format: StudioFormat;
  draft: Blueprint;
  setDraft: (next: Blueprint) => void;
  busy: boolean;
  onSave: (approve: boolean) => void;
}) {
  const prefix = format === 'article' ? 'fig' : format === 'video' ? 'shot' : 'card';
  function addSlot() {
    const n = draft.slots.length + 1;
    setDraft({
      ...draft,
      slots: [
        ...draft.slots,
        {
          slotId: `${prefix}-${n}`,
          label: format === 'article' ? `配图 ${n}` : format === 'video' ? `镜头 ${n}` : `第 ${n} 页`,
          text: '',
          visualPrompt: '',
          durationSec: format === 'video' ? 4 : null,
          status: 'pending',
          versions: [],
        },
      ],
    });
  }

  return (
    <div className="blueprint-editor">
      <label className="field-label">
        {format === 'article' ? '导语摘要（可选）' : '发布标题与短配文'}
        <textarea
          className="instructions-editor compact-textarea"
          value={draft.summaryCopy}
          onChange={e => setDraft({ ...draft, summaryCopy: e.target.value })}
          placeholder={format === 'article' ? '公众号摘要导语' : '小红书／抖音发布标题、正文与话题标签'}
        />
      </label>

      {(format === 'article' || format === 'video') && (
        <label className="field-label">
          {format === 'article' ? '定稿 Markdown 正文（插图位可用 ![图注](slot:fig-1) 嵌入）' : '完整口播／旁白逐字稿'}
          <textarea
            className="instructions-editor"
            value={draft.bodyMarkdown}
            onChange={e => setDraft({ ...draft, bodyMarkdown: e.target.value })}
            placeholder={format === 'article' ? '# 文章标题\n\n正文内容…\n\n![核心结构图](slot:fig-1)' : '前 3 秒钩子台词…'}
          />
        </label>
      )}

      <div className="slots-editor-header">
        <strong>{format === 'article' ? '封面与插图锚点槽位' : format === 'video' ? '分镜镜头槽位' : '分页卡片槽位'}</strong>
        <button className="text-button" type="button" onClick={addSlot}>＋ 新增槽位</button>
      </div>

      {draft.slots.map((slot, idx) => (
        <div key={`${slot.slotId}-${idx}`} className="slot-edit-card">
          <div className="slot-edit-row">
            <input
              value={slot.slotId}
              onChange={e => setDraft({ ...draft, slots: draft.slots.map((s, i) => i === idx ? { ...s, slotId: e.target.value.toLowerCase().replace(/[^a-z0-9-_]/g, '') } : s) })}
              placeholder="槽位 ID (如 card-1)"
            />
            <input
              value={slot.label}
              onChange={e => setDraft({ ...draft, slots: draft.slots.map((s, i) => i === idx ? { ...s, label: e.target.value } : s) })}
              placeholder="标题"
            />
            {format === 'video' && (
              <input
                type="number"
                min={1}
                max={600}
                value={slot.durationSec ?? ''}
                onChange={e => setDraft({ ...draft, slots: draft.slots.map((s, i) => i === idx ? { ...s, durationSec: e.target.value ? Number(e.target.value) : null } : s) })}
                placeholder="秒数"
              />
            )}
            <button className="text-button" onClick={() => setDraft({ ...draft, slots: draft.slots.filter((_, i) => i !== idx) })}>移除</button>
          </div>
          <textarea
            className="instructions-editor compact-textarea"
            value={slot.text}
            onChange={e => setDraft({ ...draft, slots: draft.slots.map((s, i) => i === idx ? { ...s, text: e.target.value } : s) })}
            placeholder={format === 'video' ? '本镜对应口播台词' : format === 'article' ? '插图图注或图内文字' : '图内逐字准确文案'}
          />
          <textarea
            className="instructions-editor compact-textarea"
            value={slot.visualPrompt}
            onChange={e => setDraft({ ...draft, slots: draft.slots.map((s, i) => i === idx ? { ...s, visualPrompt: e.target.value } : s) })}
            placeholder={format === 'video' ? '画面主体、景别与运镜动作描述' : '构图、主体动作与留白描述'}
          />
        </div>
      ))}

      <div className="promote-actions">
        <button className="text-button" disabled={busy} onClick={() => onSave(false)}>仅保存草稿</button>
        <button className="text-button primary-action" disabled={busy} onClick={() => onSave(true)}>保存并确认放行蓝图</button>
      </div>
    </div>
  );
}

interface SharedSlotProps {
  job: CreationJob;
  copiedKey: string;
  onCopy: (key: string, text: string) => Promise<void>;
  selectedVersionForSlot: (slot: BlueprintSlot) => ArtifactVersion | undefined;
  setSlotVersions: React.Dispatch<React.SetStateAction<Record<string, number>>>;
  artifactUrl: (ownerId: string, rel: string) => string;
  setEnlarged: (url: string) => void;
  revisingSlot: string;
  setRevisingSlot: (slotId: string) => void;
  revisionNote: string;
  setRevisionNote: (note: string) => void;
  onRenderSlot: (slotId: string, note?: string) => Promise<void>;
}

function SlotDeliverableCard({ slot, props, actionLabel }: { slot: BlueprintSlot; props: SharedSlotProps; actionLabel: string }) {
  const ver = props.selectedVersionForSlot(slot);
  const running = slot.status === 'running';
  const isRevising = props.revisingSlot === slot.slotId;

  return (
    <article className="generation-task slot-card">
      <header>
        <span className={`task-dot ${running ? 'running' : slot.status === 'error' ? 'failed' : ver ? 'done' : ''}`} />
        <strong className="task-title">{slot.label}</strong>
        <code className="slot-code">{slot.slotId}</code>
        {slot.durationSec ? <span className="badge">{slot.durationSec}s</span> : null}
        <small>{running ? '生成中…' : ver ? `已出图 · v${ver.version}` : '待生成'}</small>
      </header>

      {slot.text && <p className="slot-copy">{slot.text}</p>}
      {slot.visualPrompt && <p className="slot-visual">{slot.visualPrompt}</p>}
      {slot.error && <p className="inline-error" role="alert">{slot.error}</p>}

      {ver && (
        <div className="slot-artifact-box">
          <button className="slot-image-btn" aria-label={`放大 ${slot.label}`} onClick={() => props.setEnlarged(props.artifactUrl(props.job.id, ver.path))}>
            <img src={props.artifactUrl(props.job.id, ver.path)} alt={`${slot.label} v${ver.version}`} loading="lazy" />
          </button>
          {slot.versions.length > 1 && (
            <div className="version-pills" role="group" aria-label="历史版本切换">
              {slot.versions.map(v => (
                <button
                  key={v.version}
                  className={v.version === ver.version ? 'active' : ''}
                  onClick={() => props.setSlotVersions(old => ({ ...old, [slot.slotId]: v.version }))}
                >
                  v{v.version}
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      {isRevising ? (
        <div className="slot-revise-bar">
          <input
            autoFocus
            value={props.revisionNote}
            onChange={e => props.setRevisionNote(e.target.value)}
            placeholder="说明基于当前原图要微调哪处细节…"
            onKeyDown={e => { if (e.key === 'Enter') void props.onRenderSlot(slot.slotId, props.revisionNote); }}
          />
          <button className="text-button primary-action" onClick={() => { void props.onRenderSlot(slot.slotId, props.revisionNote); }}>生成新版</button>
          <button className="text-button" onClick={() => props.setRevisingSlot('')}>取消</button>
        </div>
      ) : (
        <div className="slot-card-actions">
          {!running && (
            ver ? (
              <button className="text-button" onClick={() => { props.setRevisingSlot(slot.slotId); props.setRevisionNote(''); }}>
                基于 v{ver.version} 微调
              </button>
            ) : (
              <button className="text-button primary-action" onClick={() => { void props.onRenderSlot(slot.slotId); }}>
                {actionLabel}
              </button>
            )
          )}
        </div>
      )}
    </article>
  );
}

function CardsCanvas(props: SharedSlotProps) {
  const { job } = props;
  return (
    <div className="canvas-scroll">
      {job.blueprint.summaryCopy && (
        <section className="canvas-copy-card">
          <header>
            <span className="sample-kicker">小红书／抖音发布配文</span>
            <button className="text-button" onClick={() => { void props.onCopy('summary', job.blueprint.summaryCopy); }}>
              {props.copiedKey === 'summary' ? '已复制' : '复制配文'}
            </button>
          </header>
          <p>{job.blueprint.summaryCopy}</p>
        </section>
      )}
      {job.blueprint.slots.map(slot => (
        <SlotDeliverableCard key={slot.slotId} slot={slot} props={props} actionLabel="生成本页卡片" />
      ))}
    </div>
  );
}

function ArticleCanvas(props: SharedSlotProps & { view: 'preview' | 'markdown'; setView: (v: 'preview' | 'markdown') => void }) {
  const { job, view, setView } = props;
  return (
    <div className="canvas-scroll">
      {job.blueprint.bodyMarkdown && (
        <section className="canvas-article-card">
          <header className="article-toolbar">
            <div className="segmented-pills">
              <button className={view === 'preview' ? 'active' : ''} onClick={() => setView('preview')}>排版预览</button>
              <button className={view === 'markdown' ? 'active' : ''} onClick={() => setView('markdown')}>Markdown</button>
            </div>
            <span className="header-space" />
            <button className="text-button" onClick={() => { void props.onCopy('article', job.blueprint.bodyMarkdown); }}>
              {props.copiedKey === 'article' ? '已复制全文' : '复制 Markdown'}
            </button>
          </header>
          {view === 'markdown' ? (
            <pre className="rules-document">{job.blueprint.bodyMarkdown}</pre>
          ) : (
            <div className="article-preview-body">
              <Markdown
                skipHtml
                urlTransform={url => url}
                components={{
                  img: ({ src, alt }) => {
                    if (src?.startsWith('slot:')) {
                      const id = src.slice(5);
                      const slot = job.blueprint.slots.find(s => s.slotId === id);
                      const ver = slot ? props.selectedVersionForSlot(slot) : undefined;
                      if (ver) {
                        return (
                          <figure className="inline-slot-figure">
                            <img src={props.artifactUrl(job.id, ver.path)} alt={alt || slot?.label || id} />
                            {alt && <figcaption>{alt}</figcaption>}
                          </figure>
                        );
                      }
                      return <span className="inline-slot-placeholder">［待生成配图：{alt || id}］</span>;
                    }
                    return null;
                  },
                }}
              >
                {job.blueprint.bodyMarkdown}
              </Markdown>
            </div>
          )}
        </section>
      )}
      {job.blueprint.slots.map(slot => (
        <SlotDeliverableCard key={slot.slotId} slot={slot} props={props} actionLabel="生成此配图" />
      ))}
    </div>
  );
}

function VideoCanvas(props: SharedSlotProps) {
  const { job } = props;
  const totalSec = job.blueprint.slots.reduce((acc, s) => acc + (s.durationSec || 0), 0);
  const promptPack = job.blueprint.slots
    .map((s, idx) => `【镜头 ${idx + 1}｜${s.label}${s.durationSec ? `｜${s.durationSec}s` : ''}】\n台词：${s.text || '—'}\n画面与运镜：${s.visualPrompt || '—'}`)
    .join('\n\n');

  return (
    <div className="canvas-scroll">
      {(job.blueprint.bodyMarkdown || job.blueprint.summaryCopy) && (
        <section className="canvas-copy-card">
          <header>
            <span className="sample-kicker">口播逐字稿{totalSec ? ` · 预计总长约 ${totalSec}s` : ''}</span>
            <div className="gate-actions">
              {job.blueprint.bodyMarkdown && (
                <button className="text-button" onClick={() => { void props.onCopy('script', job.blueprint.bodyMarkdown); }}>
                  {props.copiedKey === 'script' ? '已复制口播稿' : '复制口播稿'}
                </button>
              )}
              {job.blueprint.slots.length > 0 && (
                <button className="text-button" onClick={() => { void props.onCopy('pack', promptPack); }}>
                  {props.copiedKey === 'pack' ? '已复制分镜包' : '复制分镜提示词包'}
                </button>
              )}
            </div>
          </header>
          {job.blueprint.summaryCopy && <p className="video-caption">{job.blueprint.summaryCopy}</p>}
          {job.blueprint.bodyMarkdown && <pre className="prompt-body">{job.blueprint.bodyMarkdown}</pre>}
        </section>
      )}
      {job.blueprint.slots.map(slot => (
        <SlotDeliverableCard key={slot.slotId} slot={slot} props={props} actionLabel="生成关键帧参考图" />
      ))}
    </div>
  );
}
