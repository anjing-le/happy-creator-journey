import { useEffect, useRef, useState } from 'react';
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import Markdown from 'react-markdown';
import { Choice } from './Choice';
import { StudioEntrance, StudioLaunch } from './navigation';
import { AssetEntry, AssetLibrary, MODALITY_LABELS, Modality, loadLibrary, loadLibraryToken } from './library';

interface AdapterStatus {
  id: string;
  displayName: string;
  available: boolean;
  capabilities: string[];
  detail?: string | null;
}

interface Message { id: string; role: string; text: string }

interface CreationJob {
  parentId?: string | null;
  sourceTaskId?: string | null;
  id: string;
  title: string;
  mode: string;
  adapterId: string;
  skillId: string;
  skillName: string;
  promptIds: string[];
  snapshotSeq: number;
  draftMarkdown: string;
  status: string;
  messages: Message[];
  images: string[];
  error?: string | null;
  turnId?: string | null;
  activity: string;
}

const native = isTauri();
const busy = (j?: CreationJob) => !!j && ['starting', 'running'].includes(j.status);
const errText = (e: unknown) => e instanceof Error ? e.message : String(e);

export function Creation({ name, launch, back }: { name: StudioEntrance; launch?: StudioLaunch; back: () => void }) {
  const mode = name === '视频' ? 'video' : 'image_text';
  const [builtins, setBuiltins] = useState<AssetEntry[]>([]);
  const [library, setLibrary] = useState<AssetLibrary>({ entries: [], imageDirectory: '', tags: [], stateToken: '' });
  const [adapters, setAdapters] = useState<AdapterStatus[]>([]);
  const [adapterId, setAdapterId] = useState('codex');
  const [jobs, setJobs] = useState<CreationJob[]>([]);
  const [selectedId, setSelectedId] = useState(() => launch ? '' : (localStorage.getItem(`creatorx.studio.${mode}`) || ''));
  const [skillId, setSkillId] = useState(launch?.skillId || '');
  const [promptIds, setPromptIds] = useState<string[]>(launch?.promptId ? [launch.promptId] : []);
  const [attachmentsDirty, setAttachmentsDirty] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerModality, setPickerModality] = useState<'all' | Modality>('all');
  const [pickerTag, setPickerTag] = useState('');
  const [text, setText] = useState('');
  const [directory, setDirectory] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [enlarged, setEnlarged] = useState<string>();

  const zoomRef = useRef<HTMLDialogElement>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const followRef = useRef(true);
  const hydratedRef = useRef('');
  const handledLaunchRef = useRef<number | undefined>(undefined);
  const tokenRef = useRef('');
  const jobsRef = useRef(jobs); jobsRef.current = jobs;
  const submittingRef = useRef(submitting); submittingRef.current = submitting;

  const mainJobs = jobs.filter(j => !j.parentId && j.mode === mode);
  const job = mainJobs.find(j => j.id === selectedId);
  const childTasks = jobs.filter(j => j.parentId === job?.id);
  const allSkills = [...builtins, ...library.entries.filter(e => e.kind === 'skill')];
  const allPrompts = library.entries.filter(e => e.kind === 'prompt');
  const pickerPrompts = allPrompts.filter(p => (pickerModality === 'all' || p.modality === pickerModality) && (!pickerTag || p.tags.includes(pickerTag)));

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
      setAdapters(await invoke<AdapterStatus[]>('adapter_list', { verifyId: verifyId ?? null }));
    } catch (e) { setError(errText(e)); }
  }

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const syncAssets = async () => {
      try {
        const lib = await loadLibrary();
        if (!disposed) {
          tokenRef.current = lib.stateToken;
          setLibrary(lib);
        }
      } catch (e) {
        if (!disposed) setError(errText(e));
      }
    };
    void invoke<AssetEntry[]>('creation_builtin_skills').then(b => { if (!disposed) setBuiltins(b); }).catch(e => { if (!disposed) setError(errText(e)); });
    void syncAssets();
    void probeAdapters('codex');
    void refreshJobs().catch(e => setError(errText(e)));
    void invoke<string>('creation_directory').then(d => { if (!disposed) setDirectory(d); }).catch(e => setError(errText(e)));
    const onFocus = () => { void syncAssets(); void refreshJobs().catch(e => setError(errText(e))); };
    window.addEventListener('focus', onFocus);
    const timer = setInterval(() => {
      if (disposed || document.visibilityState !== 'visible') return;
      if (submittingRef.current || jobsRef.current.some(busy)) {
        void refreshJobs().catch(e => setError(errText(e)));
      }
      void loadLibraryToken().then(tok => {
        if (tok && tok !== tokenRef.current) void syncAssets();
      }).catch(() => {});
    }, 1000);
    return () => { disposed = true; clearInterval(timer); window.removeEventListener('focus', onFocus); };
  }, []);

  useEffect(() => {
    if (launch?.nonce && handledLaunchRef.current !== launch.nonce) {
      handledLaunchRef.current = launch.nonce;
      hydratedRef.current = '';
      setSelectedId('');
      setSkillId(launch.skillId || '');
      setPromptIds(launch.promptId ? [launch.promptId] : []);
      setAttachmentsDirty(false);
      composerRef.current?.focus();
    }
  }, [launch?.nonce]);

  useEffect(() => {
    if (!launch) localStorage.setItem(`creatorx.studio.${mode}`, selectedId);
  }, [selectedId, mode, launch]);

  useEffect(() => {
    if (job && hydratedRef.current !== job.id) {
      hydratedRef.current = job.id;
      setAdapterId(job.adapterId || 'codex');
      setSkillId(job.skillId || '');
      setPromptIds(job.promptIds || []);
      setAttachmentsDirty(false);
    }
  }, [job?.id]);

  useEffect(() => { if (enlarged) zoomRef.current?.showModal(); else zoomRef.current?.close(); }, [enlarged]);
  const lastMsg = job?.messages.at(-1)?.text;
  useEffect(() => {
    const el = messagesRef.current;
    if (el && followRef.current) el.scrollTop = el.scrollHeight;
  }, [lastMsg, selectedId]);

  function selectJob(target?: CreationJob) {
    hydratedRef.current = target?.id || '';
    setSelectedId(target?.id || '');
    setAdapterId(target?.adapterId || 'codex');
    setSkillId(target?.skillId || '');
    setPromptIds(target?.promptIds || []);
    setAttachmentsDirty(false);
    setPickerOpen(false);
    setText('');
    setError('');
    followRef.current = true;
    composerRef.current?.focus();
  }

  async function send() {
    const message = text.trim();
    if (!message || submitting || busy(job)) return;
    setSubmitting(true); setError(''); followRef.current = true;
    const prevIds = new Set(jobs.map(j => j.id));
    try {
      const result = job
        ? await invoke<CreationJob>('creation_continue', {
            id: job.id,
            text: message,
            attachments: attachmentsDirty ? { skillId, promptIds } : null,
            adapterId,
          })
        : await invoke<CreationJob>('creation_start', {
            mode,
            adapterId,
            skillId: skillId || null,
            promptIds,
            brief: message,
          });
      setSelectedId(result.id);
      setAttachmentsDirty(false);
      setPickerOpen(false);
      setText(cur => cur.trim() === message ? '' : cur);
      await refreshJobs();
    } catch (e) {
      setError(errText(e));
      const latest = await refreshJobs().catch(() => []);
      if (!job) {
        const created = latest.find(j => !prevIds.has(j.id) && j.mode === mode && !j.parentId);
        if (created) setSelectedId(created.id);
      }
    } finally {
      setSubmitting(false);
      composerRef.current?.focus();
    }
  }

  const imageSrc = (ownerId: string, rel: string) => convertFileSrc(`${directory}/${ownerId}/${rel}`);
  const locked = submitting || busy(job);
  const activeAdapter = adapters.find(a => a.id === adapterId);
  const attachedPrompts = allPrompts.filter(p => promptIds.includes(p.id));

  async function openSandboxFile(path?: string) {
    if (!path || !job) return;
    try { await invoke('creation_open_file', { id: job.id, path }); }
    catch (e) { setError(errText(e)); }
  }

  return (
    <section className="page chat-page" aria-label={`${name}创作台`}>
      <header className="page-header chat-header">
        <button className="icon-button" aria-label="返回上一页" onClick={back}>‹</button>
        <span className="studio-mode-pill">{name}</span>
        <Choice
          className="chat-session-select"
          label="切换创作会话"
          value={job?.id || ''}
          options={[{ value: '', label: '新对话' }, ...[...mainJobs].reverse().map(j => ({ value: j.id, label: j.title }))]}
          onChange={id => selectJob(mainJobs.find(j => j.id === id))}
        />
        <span className="header-space" />
        <Choice
          className="adapter-select"
          label="切换本地 Agent"
          value={adapterId}
          options={(adapters.length ? adapters : [{ id: 'codex', displayName: 'Codex · 6.1 Sol · 中等', available: true, capabilities: [] }]).map(a => ({
            value: a.id,
            label: a.displayName,
          }))}
          onChange={next => { setAdapterId(next); void probeAdapters(next); }}
        />
        <button className="text-button codex-connection" onClick={() => { void probeAdapters(adapterId); }}>
          {activeAdapter ? (activeAdapter.available ? '已就绪' : '检查连接') : '连接中…'}
        </button>
        <button className="icon-button" aria-label="开启新对话" onClick={() => selectJob()} disabled={submitting}>＋</button>
      </header>

      <div className="chat-layout studio-layout">
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
                <p>{name === '视频' ? '和 Agent 聊聊视频想法' : '和 Agent 聊聊图文想法'}</p>
                {activeAdapter?.detail && !activeAdapter.available && <small className="adapter-hint">{activeAdapter.detail}</small>}
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
            {busy(job) && <p className="chat-activity" role="status">{job?.activity || '正在回复…'}</p>}
          </div>

          <div className="chat-composer-area">
            {(error || job?.error) && <p className="inline-error" role="alert">{error || job?.error}</p>}
            <div className="chat-composer" data-editing={text.trim() ? '' : undefined}>
              {pickerOpen && (
                <div className="chat-materials">
                  <div className="picker-filters">
                    <div className="segmented-pills">
                      {(['all', 'text', 'image', 'video'] as const).map(m => (
                        <button key={m} type="button" className={pickerModality === m ? 'active' : ''} onClick={() => setPickerModality(m)}>
                          {m === 'all' ? '全部' : MODALITY_LABELS[m]}
                        </button>
                      ))}
                    </div>
                    {library.tags.map(tag => (
                      <button
                        key={tag}
                        type="button"
                        className={`filter-chip ${pickerTag === tag ? 'active' : ''}`}
                        onClick={() => setPickerTag(pickerTag === tag ? '' : tag)}
                      >
                        #{tag}
                      </button>
                    ))}
                  </div>
                  <div className="picker-list">
                    {pickerPrompts.length === 0 ? (
                      <p className="sidebar-note">样板间暂无对应提示词</p>
                    ) : pickerPrompts.map(p => {
                      const active = promptIds.includes(p.id);
                      return (
                        <button
                          key={p.id}
                          type="button"
                          disabled={locked || (!active && promptIds.length >= 16)}
                          className={`picker-item ${active ? 'selected' : ''}`}
                          onClick={() => {
                            setPromptIds(old => active ? old.filter(id => id !== p.id) : [...old, p.id]);
                            setAttachmentsDirty(true);
                          }}
                        >
                          <span className="choice-check" aria-hidden="true">{active ? '✓' : ''}</span>
                          <span className="badge format">{MODALITY_LABELS[p.modality]}</span>
                          <span className="picker-name">{p.name}</span>
                          {p.tags.map(t => <small key={t} className="badge">#{t}</small>)}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {attachedPrompts.length > 0 && (
                <div className="attached-chips">
                  {attachedPrompts.map(p => (
                    <span key={p.id} className="tag-pill">
                      [{MODALITY_LABELS[p.modality]}] {p.name}
                      {!locked && (
                        <button
                          type="button"
                          aria-label={`取消引用 ${p.name}`}
                          onClick={() => { setPromptIds(old => old.filter(id => id !== p.id)); setAttachmentsDirty(true); }}
                        >
                          ×
                        </button>
                      )}
                    </span>
                  ))}
                </div>
              )}

              <textarea
                ref={composerRef}
                autoFocus
                aria-label="与 Agent 交流"
                value={text}
                rows={3}
                onChange={e => setText(e.target.value)}
                onKeyDown={e => {
                  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter' && !e.nativeEvent.isComposing) {
                    e.preventDefault();
                    void send();
                  }
                }}
                placeholder="输入想法或修改意见…"
              />
              <div className="chat-composer-toolbar">
                <button
                  className={`text-button context-btn ${pickerOpen ? 'active' : ''}`}
                  aria-label="引用提示词"
                  aria-expanded={pickerOpen}
                  onClick={() => setPickerOpen(!pickerOpen)}
                >
                  ＋ 提示词{promptIds.length ? ` · ${promptIds.length}` : ''}
                </button>
                <Choice
                  className="composer-skill"
                  label="选择 Skill"
                  value={skillId}
                  disabled={locked}
                  options={[
                    { value: '', label: '选择 Skill' },
                    ...builtins.map(s => ({ value: s.id, label: s.name, group: '内置' })),
                    ...allSkills.filter(s => !s.id.startsWith('builtin:')).map(s => ({ value: s.id, label: s.name, group: '我的 Skill' })),
                  ]}
                  onChange={id => { setSkillId(id); setAttachmentsDirty(true); }}
                />
                <span className="header-space" />
                {busy(job) ? (
                  <button
                    className="chat-send stop"
                    aria-label="停止"
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

        <aside className="creation-results harness-canvas" aria-label="作品展示">
          <div className="canvas-scroll">
            {!job || (!job.draftMarkdown.trim() && childTasks.length === 0) ? (
              <div className="results-empty">
                <span aria-hidden="true">◇</span>
                <p>作品会呈现在这里</p>
              </div>
            ) : (
              <>
                {job.draftMarkdown.trim() && (
                  <section className="canvas-copy-card">
                    <pre className="prompt-body">{job.draftMarkdown}</pre>
                  </section>
                )}

                {childTasks.map(task => (
                  <article className="generation-task slot-card" key={task.id}>
                    <header>
                      <span className={`task-dot ${busy(task) ? 'running' : task.error ? 'failed' : 'done'}`} />
                      <strong className="task-title">{task.title}</strong>
                      <small className="slot-id-note">{task.id.slice(0, 8)}</small>
                    </header>
                    {busy(task) && <p className="chat-activity" role="status">{task.activity || '生成中…'}</p>}
                    {task.error && <p className="inline-error" role="alert">{task.error}</p>}
                    {task.images.length > 0 && (
                      <div className="result-images">
                        {task.images.map((rel, i) => (
                          <button key={rel} className="slot-image-btn" aria-label={`查看原图 ${i + 1}`} onClick={() => setEnlarged(imageSrc(task.id, rel))}>
                            <img src={imageSrc(task.id, rel)} alt={`作品 ${i + 1}`} loading="lazy" />
                          </button>
                        ))}
                      </div>
                    )}
                  </article>
                ))}
              </>
            )}
          </div>
        </aside>
      </div>

      <dialog ref={zoomRef} className="image-lightbox" aria-label="查看作品原图" onCancel={e => { e.preventDefault(); setEnlarged(undefined); }} onClick={e => { if (e.target === e.currentTarget) setEnlarged(undefined); }}>
        <button className="lightbox-close icon-button" aria-label="关闭原图" onClick={() => setEnlarged(undefined)}>×</button>
        {enlarged && <img src={enlarged} alt="作品原图" />}
      </dialog>
    </section>
  );
}
