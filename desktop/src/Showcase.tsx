import { useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { Choice } from './Choice';
import { StudioEntrance, StudioLaunch } from './navigation';
import {
  AssetEntry,
  AssetKind,
  AssetLibrary,
  DELIVERABLE_LABELS,
  DeliverableFormat,
  IpFoundation,
  Maturity,
  SkillFile,
  Track,
  deleteEntry,
  emptyEntry,
  emptyIp,
  imageUrl,
  importImage,
  loadLibrary,
  openSkillFolder,
  readSkillFolder,
  saveEntry,
  saveIp,
  skillFiles,
} from './library';

type Section = 'ip' | 'skill' | 'prompt';

const native = isTauri();
const PROMPT_PRESET_CATEGORIES = ['视觉画风', '写作母版', '脚本钩子'];
const SKILL_PRESET_CATEGORIES = ['卡片图文', '长文配图', '视频创作'];

function BackIcon() {
  return <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m14 6-6 6 6 6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>;
}
function PlusIcon() {
  return <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 5v14M5 12h14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>;
}
function errText(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}

function launchTargetForEntry(entry: AssetEntry): { entrance: StudioEntrance; launch: StudioLaunch } {
  const format = entry.deliverable === 'video' ? 'video' : entry.deliverable === 'article' ? 'article' : 'cards';
  const entrance: StudioEntrance = format === 'video' ? '视频' : '图文';
  return {
    entrance,
    launch: {
      deliverable: format,
      skillId: entry.kind === 'skill' ? entry.id : undefined,
      promptId: entry.kind === 'prompt' ? entry.id : undefined,
      track: entry.track || undefined,
      nonce: Date.now(),
    },
  };
}

export function Showcase({ back, onLaunch }: { back: () => void; onLaunch: (entrance: StudioEntrance, launch: StudioLaunch) => void }) {
  const [library, setLibrary] = useState<AssetLibrary>({ ip: emptyIp(), entries: [], imageDirectory: '', categories: [], tracks: [] });
  const [builtins, setBuiltins] = useState<AssetEntry[]>([]);
  const [section, setSection] = useState<Section>('skill');
  const [selected, setSelected] = useState<AssetEntry>();
  const [editingEntry, setEditingEntry] = useState(false);
  const [editingIp, setEditingIp] = useState(false);
  const [deliverableFilter, setDeliverableFilter] = useState<string>('all');
  const [categoryFilter, setCategoryFilter] = useState<string>('all');
  const [trackFilter, setTrackFilter] = useState<string>('all');
  const [maturityFilter, setMaturityFilter] = useState<Maturity>('showcase');
  const [query, setQuery] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const busyEditing = editingEntry || editingIp;
  const busyRef = useRef(false);
  busyRef.current = busyEditing;

  async function refresh() {
    if (busyRef.current) return;
    try {
      const [lib, built] = await Promise.all([
        loadLibrary(),
        native ? invoke<AssetEntry[]>('creation_builtin_skills') : Promise.resolve([]),
      ]);
      setLibrary(lib);
      setBuiltins(built);
      setSelected(old => {
        if (!old) return undefined;
        return [...built, ...lib.entries].find(item => item.id === old.id);
      });
      setError('');
    } catch (e) {
      setError(errText(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
    const onFocus = () => { void refresh(); };
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, []);

  if (editingIp) {
    return (
      <IpEditor
        ip={library.ip}
        directory={library.imageDirectory}
        cancel={() => setEditingIp(false)}
        saved={async next => {
          busyRef.current = false;
          setEditingIp(false);
          setLibrary(old => ({ ...old, ip: next }));
          await refresh();
        }}
      />
    );
  }

  if (selected && editingEntry) {
    return (
      <ShowcaseEditor
        entry={selected}
        directory={library.imageDirectory}
        categories={library.categories}
        tracks={library.tracks}
        cancel={() => {
          setEditingEntry(false);
          if (!selected.id) setSelected(undefined);
        }}
        saved={async saved => {
          busyRef.current = false;
          setEditingEntry(false);
          await refresh();
          setSelected(saved);
        }}
      />
    );
  }

  const allSkills = [...builtins, ...library.entries.filter(e => e.kind === 'skill')];
  const allPrompts = library.entries.filter(e => e.kind === 'prompt');
  const currentPool = section === 'skill' ? allSkills : allPrompts;
  const draftCount = currentPool.filter(e => e.maturity === 'draft').length;
  const showcaseCount = currentPool.filter(e => e.maturity !== 'draft').length;

  const promptCategories = [...new Set([...PROMPT_PRESET_CATEGORIES, ...allPrompts.map(e => e.category).filter(Boolean)])];
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);

  const filtered = currentPool.filter(entry => {
    const entryMaturity = entry.maturity || 'showcase';
    if (entryMaturity !== maturityFilter) return false;
    if (section === 'skill' && deliverableFilter !== 'all' && entry.deliverable !== deliverableFilter) return false;
    if (section === 'prompt' && categoryFilter !== 'all' && (entry.category || '') !== categoryFilter) return false;
    if (trackFilter !== 'all' && (entry.track || '') !== trackFilter) return false;
    if (!terms.length) return true;
    const hay = `${entry.name} ${entry.category} ${entry.track} ${entry.sampleInput} ${entry.sampleOutput} ${entry.prompt} ${entry.skillMarkdown} ${entry.notes}`.toLocaleLowerCase();
    return terms.every(t => hay.includes(t));
  });

  const startCreate = (kind: AssetKind) => {
    const presetDeliverable: DeliverableFormat = section === 'skill' && deliverableFilter !== 'all'
      ? (deliverableFilter as DeliverableFormat)
      : kind === 'skill' ? 'cards' : 'general';
    const draft = emptyEntry(kind, presetDeliverable);
    if (kind === 'prompt' && categoryFilter !== 'all') draft.category = categoryFilter;
    if (trackFilter !== 'all') draft.track = trackFilter;
    draft.maturity = maturityFilter;
    setSelected(draft);
    setEditingEntry(true);
  };

  return (
    <section className="page showcase-page" aria-label="样板间与资产底座">
      <header className="page-header showcase-header">
        <button className="icon-button" onClick={back} aria-label="返回首页"><BackIcon /></button>
        <nav className="library-tabs compact-tabs" aria-label="资产分区">
          <button className={section === 'ip' ? 'active' : ''} onClick={() => { setSection('ip'); setSelected(undefined); }}>
            IP 底座<span className="tab-count">{library.ip.name || '待设'}</span>
          </button>
          <button className={section === 'skill' ? 'active' : ''} onClick={() => { setSection('skill'); setSelected(undefined); }}>
            制式 Skills<span className="tab-count">{allSkills.length}</span>
          </button>
          <button className={section === 'prompt' ? 'active' : ''} onClick={() => { setSection('prompt'); setSelected(undefined); }}>
            风格提示词<span className="tab-count">{allPrompts.length}</span>
          </button>
        </nav>
        <span className="header-space" />
        {section !== 'ip' && (
          <div className="showcase-toolbar-actions">
            <Choice
              className="showcase-track-select"
              label="按赛道筛选"
              value={trackFilter}
              options={[
                { value: 'all', label: '全部赛道' },
                { value: '', label: '通用赛道' },
                ...library.tracks.map(t => ({ value: t, label: t })),
              ]}
              onChange={setTrackFilter}
            />
            <div className="segmented-pills" role="group" aria-label="资产成熟度">
              <button className={maturityFilter === 'showcase' ? 'active' : ''} onClick={() => { setMaturityFilter('showcase'); setSelected(undefined); }}>
                样板 {showcaseCount}
              </button>
              <button className={maturityFilter === 'draft' ? 'active' : ''} onClick={() => { setMaturityFilter('draft'); setSelected(undefined); }}>
                草稿 {draftCount}
              </button>
            </div>
            <input
              className="showcase-search"
              aria-label="搜索样板"
              placeholder="搜索名称或样例…"
              value={query}
              onChange={e => setQuery(e.target.value)}
            />
            <button className="text-button primary-action" onClick={() => startCreate(section === 'skill' ? 'skill' : 'prompt')}>
              ＋ {section === 'skill' ? '添加样板 Skill' : '添加风格提示词'}
            </button>
          </div>
        )}
      </header>

      {error && <div className="browser-error inline-error" role="alert">{error}</div>}

      {section === 'ip' ? (
        <IpShowcaseView ip={library.ip} directory={library.imageDirectory} onEdit={() => setEditingIp(true)} />
      ) : selected ? (
        <ShowcaseDetail
          entry={selected}
          directory={library.imageDirectory}
          close={() => setSelected(undefined)}
          edit={() => setEditingEntry(true)}
          useInStudio={() => {
            const target = launchTargetForEntry(selected);
            onLaunch(target.entrance, target.launch);
          }}
          deleted={async () => {
            setSelected(undefined);
            await refresh();
          }}
        />
      ) : (
        <div className="showcase-body">
          <div className="showcase-subfilters">
            {section === 'skill' ? (
              (['all', 'cards', 'article', 'video'] as const).map(key => (
                <button
                  key={key}
                  className={`filter-chip ${deliverableFilter === key ? 'active' : ''}`}
                  onClick={() => setDeliverableFilter(key)}
                >
                  {key === 'all' ? '全部制式' : DELIVERABLE_LABELS[key]}
                </button>
              ))
            ) : (
              (['all', ...promptCategories] as const).map(cat => (
                <button
                  key={cat}
                  className={`filter-chip ${categoryFilter === cat ? 'active' : ''}`}
                  onClick={() => setCategoryFilter(cat)}
                >
                  {cat === 'all' ? '全部分类' : cat}
                </button>
              ))
            )}
          </div>

          {loading ? (
            <div className="empty-library"><p>正在读取样板间…</p></div>
          ) : filtered.length === 0 ? (
            <div className="empty-library">
              <button className="empty-add" onClick={() => startCreate(section === 'skill' ? 'skill' : 'prompt')}>
                <PlusIcon />
                <span>
                  {maturityFilter === 'draft'
                    ? '当前没有待打磨草稿，点击记录一条新想法'
                    : section === 'skill'
                      ? '还没有符合筛选的制式样板 Skill，点击添加第一套样板间'
                      : '还没有符合筛选的风格提示词，点击沉淀第一张风格样板'}
                </span>
              </button>
            </div>
          ) : (
            <div className="showcase-grid">
              {filtered.map(entry => {
                const isBuiltin = entry.id.startsWith('builtin:');
                const previewText = entry.sampleOutput.trim() || entry.notes.trim() || (entry.kind === 'prompt' ? entry.prompt : entry.skillMarkdown);
                return (
                  <article key={entry.id} className="showcase-card">
                    <button className="showcase-card-cover" onClick={() => setSelected(entry)} aria-label={`查看样板 ${entry.name}`}>
                      {entry.images.length > 0 ? (
                        <div className="showcase-cover-art">
                          {entry.images.length > 1 && <span className="series-sheet" aria-hidden="true" />}
                          <img src={imageUrl(library.imageDirectory, entry.images[0])} alt={entry.name} loading="lazy" />
                          {entry.images.length > 1 && <span className="image-count-badge">{entry.images.length} 张样例</span>}
                        </div>
                      ) : (
                        <div className="showcase-cover-text">
                          <span className="sample-kicker">{entry.sampleOutput.trim() ? '效果样板' : '规范摘要'}</span>
                          <p>{previewText.slice(0, 140)}</p>
                        </div>
                      )}
                    </button>
                    <div className="showcase-card-meta">
                      <div className="showcase-badges">
                        <span className="badge format">{DELIVERABLE_LABELS[entry.deliverable || 'general']}</span>
                        {entry.category && entry.category !== DELIVERABLE_LABELS[entry.deliverable] && <span className="badge">{entry.category}</span>}
                        <span className="badge track">{entry.track || '通用'}</span>
                        {isBuiltin && <span className="badge builtin">内置</span>}
                        {entry.maturity === 'draft' && <span className="badge draft">草稿</span>}
                      </div>
                      <div className="showcase-card-footer">
                        <button className="showcase-title-btn" onClick={() => setSelected(entry)}>{entry.name}</button>
                        <button
                          className="text-button use-template-btn"
                          onClick={() => {
                            const target = launchTargetForEntry(entry);
                            onLaunch(target.entrance, target.launch);
                          }}
                        >
                          去创作 →
                        </button>
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function IpShowcaseView({ ip, directory, onEdit }: { ip: IpFoundation; directory: string; onEdit: () => void }) {
  const [enlarged, setEnlarged] = useState<string>();
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { if (enlarged) dialog.current?.showModal(); else dialog.current?.close(); }, [enlarged]);

  const configured = ip.name.trim() || ip.visualPrompt.trim() || ip.voicePrompt.trim() || ip.images.length > 0;
  if (!configured) {
    return (
      <div className="showcase-body">
        <div className="ip-empty-card">
          <h2>先立住你的创作者 IP 底座</h2>
          <p>把角色视觉标准、参考图、表达口吻与内容赛道（如计算机、金融、音乐）存成唯一底座。之后无论是做小红书卡片、公众号长文还是抖音视频，Harness 都会默认装入快照，不用再翻找基础设定。</p>
          <button className="primary-action text-button" onClick={onEdit}>建立 IP 底座</button>
        </div>
      </div>
    );
  }

  return (
    <div className="showcase-body">
      <article className="ip-foundation-board">
        <header className="ip-board-header">
          <div>
            <span className="sample-kicker">创作者 IP 底座 · 默认随创作快照挂载</span>
            <h2>{ip.name || '未命名 IP'}</h2>
          </div>
          <span className="header-space" />
          <button className="text-button primary-action" onClick={onEdit}>编辑 IP 底座</button>
        </header>

        {ip.tracks.length > 0 && (
          <section className="ip-section">
            <h3>内容赛道定位</h3>
            <div className="ip-tracks-grid">
              {ip.tracks.map(t => (
                <div key={t.name} className="ip-track-card">
                  <strong>{t.name}</strong>
                  <span>{t.positioning || '持续积累该方向选题与样板'}</span>
                </div>
              ))}
            </div>
          </section>
        )}

        <div className="ip-two-col">
          <section className="ip-section">
            <h3>角色与视觉标准</h3>
            {ip.images.length > 0 && (
              <div className="ip-images-strip">
                {ip.images.map((img, i) => (
                  <button key={img} onClick={() => setEnlarged(imageUrl(directory, img))} aria-label={`放大标准参考图 ${i + 1}`}>
                    <img src={imageUrl(directory, img)} alt={`${ip.name} 参考图 ${i + 1}`} />
                  </button>
                ))}
              </div>
            )}
            <pre className="prompt-body">{ip.visualPrompt || '暂未填写视觉形象 Prompt'}</pre>
          </section>

          <section className="ip-section">
            <h3>人设与表达口吻</h3>
            <pre className="prompt-body">{ip.voicePrompt || '暂未填写表达口吻规范'}</pre>
            {ip.voiceSample && (
              <div className="sample-box">
                <span className="sample-kicker">口吻样板片段</span>
                <p>{ip.voiceSample}</p>
              </div>
            )}
          </section>
        </div>
      </article>
      <dialog ref={dialog} className="image-lightbox" onCancel={e => { e.preventDefault(); setEnlarged(undefined); }} onClick={e => { if (e.target === e.currentTarget) setEnlarged(undefined); }}>
        <button className="lightbox-close icon-button" onClick={() => setEnlarged(undefined)} aria-label="关闭大图">×</button>
        {enlarged && <img src={enlarged} alt={ip.name} />}
      </dialog>
    </div>
  );
}

function IpEditor({ ip, directory, cancel, saved }: { ip: IpFoundation; directory: string; cancel: () => void; saved: (ip: IpFoundation) => Promise<void> }) {
  const [draft, setDraft] = useState<IpFoundation>({ ...ip, images: [...ip.images], tracks: ip.tracks.map(t => ({ ...t })) });
  const [trackName, setTrackName] = useState('');
  const [trackDesc, setTrackDesc] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const fileInput = useRef<HTMLInputElement>(null);

  function fillAnjingStarter() {
    setDraft(old => ({
      ...old,
      name: old.name || '安静',
      visualPrompt: old.visualPrompt || '安静 IP 角色：蛋黄黄蓬松乱发、黑色连帽卫衣、圆脸点状豆豆眼、淡淡微笑与腮红；粗颗粒蜡笔与黑色马克笔手绘线条，暖米白纸张纹理留白，不使用写实人脸、3D 渲染或精致矢量扁平风。',
      voicePrompt: old.voicePrompt || '计算机从业者视角，克制、直白、准确。先给结论再拆解逻辑，用常见白话解释专业概念，不贩卖焦虑，不编造未经核实的经历或收益结果。',
      voiceSample: old.voiceSample || '很多人以为换个更强的模型就能解决内容跑偏，其实真正缺的不是模型智商，而是把「人设、蓝图确认和版本校验」固定下来的创作治具。',
      tracks: old.tracks.length ? old.tracks : [
        { name: '计算机', positioning: 'AI 工程实践、架构拆解与开发效率工具' },
        { name: '金融', positioning: '基础金融常识、资产配置逻辑与风险边界' },
        { name: '音乐', positioning: '日常演奏、听感记录与音乐审美分享' },
      ],
    }));
  }

  async function addImages(files: FileList | null) {
    if (!files?.length || busy) return;
    if (draft.images.length + files.length > 16) { setError('最多保存 16 张标准参考图'); return; }
    setBusy(true); setError('');
    try {
      const names: string[] = [];
      for (const f of Array.from(files)) names.push(await importImage(f));
      setDraft(old => ({ ...old, images: [...old.images, ...names] }));
    } catch (e) { setError(errText(e)); }
    finally { setBusy(false); if (fileInput.current) fileInput.current.value = ''; }
  }

  function addTrack() {
    const name = trackName.trim();
    if (!name) return;
    if (draft.tracks.some(t => t.name === name)) { setError('该赛道已存在'); return; }
    setDraft(old => ({ ...old, tracks: [...old.tracks, { name, positioning: trackDesc.trim() }] }));
    setTrackName(''); setTrackDesc(''); setError('');
  }

  async function submit() {
    if (busy) return;
    if (!native) { setError('请在 CreatorX 客户端中保存'); return; }
    setBusy(true); setError('');
    try { await saved(await saveIp(draft)); }
    catch (e) { setError(errText(e)); }
    finally { setBusy(false); }
  }

  return (
    <section className="page" data-editing aria-label="编辑 IP 底座">
      <header className="page-header">
        <button className="icon-button" onClick={cancel} disabled={busy} aria-label="取消编辑"><BackIcon /></button>
        <h1>编辑 IP 底座</h1>
        <button className="text-button" onClick={fillAnjingStarter} disabled={busy}>填入「安静」起步模板</button>
        <span className="header-space" />
        <button className="text-button save-button" onClick={() => { void submit(); }} disabled={busy || !draft.name.trim()}>
          {busy ? '保存中…' : '保存 IP 底座'}
        </button>
      </header>
      <div className="asset-editor">
        <label className="field-label">
          IP 名称
          <input autoFocus maxLength={80} value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} placeholder="例如：安静" disabled={busy} />
        </label>

        <div className="field-label">
          内容赛道（创作与样板间共用此赛道标签）
          <div className="track-editor-list">
            {draft.tracks.map((t: Track, idx: number) => (
              <div key={t.name} className="track-editor-item">
                <strong>{t.name}</strong>
                <input
                  value={t.positioning}
                  maxLength={500}
                  placeholder="一句话说明该赛道讲什么"
                  onChange={e => setDraft(old => ({ ...old, tracks: old.tracks.map((item, i) => i === idx ? { ...item, positioning: e.target.value } : item) }))}
                />
                <button className="text-button" onClick={() => setDraft(old => ({ ...old, tracks: old.tracks.filter((_, i) => i !== idx) }))}>移除</button>
              </div>
            ))}
            <div className="track-editor-add">
              <input value={trackName} maxLength={40} placeholder="新赛道名（如：计算机）" onChange={e => setTrackName(e.target.value)} />
              <input value={trackDesc} maxLength={500} placeholder="赛道定位与受众说明（可选）" onChange={e => setTrackDesc(e.target.value)} />
              <button className="text-button primary-action" type="button" onClick={addTrack} disabled={!trackName.trim()}>添加赛道</button>
            </div>
          </div>
        </div>

        <label className="field-label">
          角色与视觉形象标准 Prompt
          <textarea className="instructions-editor" value={draft.visualPrompt} onChange={e => setDraft({ ...draft, visualPrompt: e.target.value })} placeholder="描述固定角色外观、画风笔触、色彩与禁止项" disabled={busy} />
        </label>

        <div className="field-label">
          标准角色参考图（生图快照将自动附带）
          <div className="edit-images">
            {draft.images.map((name, i) => (
              <div className="edit-image" key={`${name}-${i}`}>
                <img src={imageUrl(directory, name)} alt={`参考图 ${i + 1}`} />
                <div className="image-actions">
                  <button disabled={busy || i === 0} onClick={() => setDraft(old => ({ ...old, images: [name, ...old.images.filter((_, j) => j !== i)] }))}>{i === 0 ? '主参考' : '设为主图'}</button>
                  <button disabled={busy} onClick={() => setDraft(old => ({ ...old, images: old.images.filter((_, j) => j !== i) }))}>移除</button>
                </div>
              </div>
            ))}
            <button className="add-image" disabled={busy} onClick={() => fileInput.current?.click()}><PlusIcon /><span>添加参考图</span></button>
          </div>
          <input ref={fileInput} type="file" hidden multiple accept="image/png,image/jpeg,image/webp" onChange={e => { void addImages(e.target.files); }} />
        </div>

        <label className="field-label">
          人设与表达口吻规范
          <textarea className="instructions-editor compact-textarea" value={draft.voicePrompt} onChange={e => setDraft({ ...draft, voicePrompt: e.target.value })} placeholder="描述行文风格、叙事视角、常见词汇偏好与禁忌" disabled={busy} />
        </label>

        <label className="field-label">
          表达口吻样例片段（可选）
          <textarea className="instructions-editor compact-textarea" value={draft.voiceSample} onChange={e => setDraft({ ...draft, voiceSample: e.target.value })} placeholder="贴一段最能代表你平时说话味道的真实文字" disabled={busy} />
        </label>

        {error && <p className="inline-error" role="alert">{error}</p>}
      </div>
    </section>
  );
}

function ShowcaseDetail({
  entry, directory, close, edit, useInStudio, deleted,
}: {
  entry: AssetEntry; directory: string; close: () => void; edit: () => void; useInStudio: () => void; deleted: () => Promise<void>;
}) {
  const [index, setIndex] = useState(0);
  const [enlarged, setEnlarged] = useState(false);
  const [files, setFiles] = useState<string[]>([]);
  const [copied, setCopied] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState('');
  const zoom = useRef<HTMLDialogElement>(null);
  const isBuiltin = entry.id.startsWith('builtin:');
  const isPrompt = entry.kind === 'prompt';

  useEffect(() => { if (enlarged) zoom.current?.showModal(); else zoom.current?.close(); }, [enlarged]);
  useEffect(() => {
    if (!isPrompt && !isBuiltin) void skillFiles(entry.id).then(setFiles).catch(e => setError(errText(e)));
    else if (!isPrompt && isBuiltin) setFiles(['SKILL.md']);
  }, [entry.id, isPrompt, isBuiltin]);

  async function copyRule() {
    try {
      await navigator.clipboard.writeText(isPrompt ? entry.prompt : entry.skillMarkdown);
      setCopied(true);
    } catch { setError('复制失败，请手动选中文本复制'); }
  }

  async function handleDelete() {
    try {
      await deleteEntry(entry);
      await deleted();
    } catch (e) { setError(errText(e)); }
  }

  return (
    <div className="showcase-body">
      <article className="showcase-detail-panel" aria-label={entry.name}>
        <header className="material-heading">
          <button className="icon-button" onClick={close} aria-label="返回样板列表"><BackIcon /></button>
          <div>
            <div className="showcase-badges">
              <span className="badge format">{DELIVERABLE_LABELS[entry.deliverable || 'general']}</span>
              {entry.category && <span className="badge">{entry.category}</span>}
              <span className="badge track">{entry.track || '通用赛道'}</span>
              {isBuiltin ? <span className="badge builtin">内置样板</span> : entry.maturity === 'draft' ? <span className="badge draft">待打磨草稿</span> : <span className="badge">已验证样板</span>}
            </div>
            <h2>{entry.name}</h2>
          </div>
          <span className="header-space" />
          <button className="text-button primary-action" onClick={useInStudio}>用此样板去创作 →</button>
          <button className="text-button" onClick={() => { void copyRule(); }}>{copied ? '已复制' : isPrompt ? '复制 Prompt' : '复制 SKILL.md'}</button>
          {!isPrompt && !isBuiltin && <button className="text-button" onClick={() => { void openSkillFolder(entry.id).catch(e => setError(errText(e))); }}>打开文件夹</button>}
          {!isBuiltin && <button className="text-button" onClick={edit}>编辑</button>}
          {!isBuiltin && (confirmDelete ? (
            <button className="text-button danger-button" onClick={() => { void handleDelete(); }}>确认删除</button>
          ) : (
            <button className="text-button" onClick={() => setConfirmDelete(true)}>删除</button>
          ))}
        </header>

        {entry.images.length > 0 && (
          <section className="detail-gallery">
            <button className="main-art" aria-label="放大样板图" onClick={() => setEnlarged(true)}>
              <img src={imageUrl(directory, entry.images[index])} alt={entry.name} />
            </button>
            {entry.images.length > 1 && (
              <div className="filmstrip">
                {entry.images.map((name, i) => (
                  <button key={`${name}-${i}`} aria-label={`查看样板图 ${i + 1}`} aria-pressed={i === index} onClick={() => setIndex(i)}>
                    <img src={imageUrl(directory, name)} alt={`样板图 ${i + 1}`} />
                  </button>
                ))}
              </div>
            )}
          </section>
        )}

        {(entry.sampleInput.trim() || entry.sampleOutput.trim()) && (
          <section className="showcase-sample-pair">
            {entry.sampleInput.trim() && (
              <div className="sample-box">
                <span className="sample-kicker">输入选题／场景示例</span>
                <p>{entry.sampleInput}</p>
              </div>
            )}
            {entry.sampleOutput.trim() && (
              <div className="sample-box">
                <span className="sample-kicker">代表性样板输出／适用规格</span>
                <p>{entry.sampleOutput}</p>
              </div>
            )}
          </section>
        )}

        {entry.notes && <p className="material-notes">{entry.notes}</p>}

        {!isPrompt && (
          <details className="skill-package" open>
            <summary>Skill 文件夹结构 · {files.length} 个文件</summary>
            <ul className="skill-file-tree">
              {files.map(path => <li key={path}>{path}</li>)}
            </ul>
          </details>
        )}

        <details className="rules-section" open={entry.images.length === 0 && !entry.sampleOutput.trim()}>
          <summary className="rules-toggle">{isPrompt ? '底层 Prompt 原文' : '底层 SKILL.md 规范'}</summary>
          <pre className="rules-document">{isPrompt ? entry.prompt : entry.skillMarkdown}</pre>
        </details>

        {entry.source && <p className="asset-source">来源：{entry.source}</p>}
        {error && <p className="inline-error" role="alert">{error}</p>}

        <dialog ref={zoom} className="image-lightbox" onCancel={e => { e.preventDefault(); setEnlarged(false); }} onClick={e => { if (e.target === e.currentTarget) setEnlarged(false); }}>
          <button className="lightbox-close icon-button" onClick={() => setEnlarged(false)} aria-label="关闭大图">×</button>
          {entry.images[index] && <img src={imageUrl(directory, entry.images[index])} alt={entry.name} />}
        </dialog>
      </article>
    </div>
  );
}

function ShowcaseEditor({
  entry, directory, categories, tracks, cancel, saved,
}: {
  entry: AssetEntry; directory: string; categories: string[]; tracks: string[]; cancel: () => void; saved: (entry: AssetEntry) => Promise<void>;
}) {
  const [draft, setDraft] = useState<AssetEntry>({ ...entry, images: [...entry.images] });
  const [imported, setImported] = useState<SkillFile[]>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const imageFiles = useRef<HTMLInputElement>(null);
  const folderInput = useRef<HTMLInputElement>(null);
  const isPrompt = draft.kind === 'prompt';
  const presetCats = isPrompt ? PROMPT_PRESET_CATEGORIES : SKILL_PRESET_CATEGORIES;
  const mergedCategories = [...new Set([...presetCats, ...categories])];

  async function addImages(selected: FileList | null) {
    if (!selected?.length || busy) return;
    if (draft.images.length + selected.length > 16) { setError('每份样板最多保存 16 张成品或参考图'); return; }
    setBusy(true); setError('');
    try {
      const names: string[] = [];
      for (const file of Array.from(selected)) names.push(await importImage(file));
      setDraft(old => ({ ...old, images: [...old.images, ...names] }));
    } catch (e) { setError(errText(e)); }
    finally { setBusy(false); if (imageFiles.current) imageFiles.current.value = ''; }
  }

  async function importFolder(selected: FileList | null) {
    if (!selected?.length || busy) return;
    setBusy(true); setError('');
    try {
      const result = await readSkillFolder(selected);
      setImported(result.files);
      setDraft(old => ({ ...old, name: old.name || result.name, skillMarkdown: result.markdown }));
    } catch (e) { setError(errText(e)); }
    finally { setBusy(false); if (folderInput.current) folderInput.current.value = ''; }
  }

  async function submit() {
    if (busy) return;
    if (!native) { setError('请在 CreatorX 客户端中保存'); return; }
    setBusy(true); setError('');
    try { await saved(await saveEntry(draft, imported)); }
    catch (e) { setError(errText(e)); }
    finally { setBusy(false); }
  }

  return (
    <section className="page" data-editing aria-label="编辑样板资产">
      <header className="page-header">
        <button className="icon-button" onClick={cancel} disabled={busy} aria-label="取消编辑"><BackIcon /></button>
        <h1>{entry.id ? '编辑样板资产' : isPrompt ? '添加风格提示词样板' : '添加制式样板 Skill'}</h1>
        <span className="header-space" />
        <button
          className="text-button save-button"
          onClick={() => { void submit(); }}
          disabled={busy || !draft.name.trim() || !(isPrompt ? draft.prompt : draft.skillMarkdown).trim()}
        >
          {busy ? '保存中…' : '保存'}
        </button>
      </header>
      <div className="asset-editor">
        <label className="field-label">
          样板名称
          <input autoFocus maxLength={80} value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} placeholder={isPrompt ? '例如：安静粗蜡笔技术架构图风' : '例如：安静小红书金融科普 5 页卡'} disabled={busy} />
        </label>

        <div className="editor-four-grid">
          <label className="field-label">
            目标交付制式
            <Choice
              label="目标交付制式"
              value={draft.deliverable}
              options={(isPrompt ? (['general', 'cards', 'article', 'video'] as const) : (['cards', 'article', 'video'] as const)).map(v => ({
                value: v,
                label: DELIVERABLE_LABELS[v],
              }))}
              onChange={v => setDraft({ ...draft, deliverable: v as DeliverableFormat })}
              disabled={busy}
            />
          </label>
          <label className="field-label">
            资产分类
            <Choice
              label="资产分类"
              editable
              placeholder="选择或输入分类"
              value={draft.category}
              options={mergedCategories.map(c => ({ value: c, label: c }))}
              onChange={category => setDraft({ ...draft, category })}
              disabled={busy}
            />
          </label>
          <label className="field-label">
            适用赛道
            <Choice
              label="适用赛道"
              editable
              placeholder="留空表示通用赛道"
              value={draft.track}
              options={[{ value: '', label: '通用' }, ...tracks.map(t => ({ value: t, label: t }))]}
              onChange={track => setDraft({ ...draft, track })}
              disabled={busy}
            />
          </label>
          <label className="field-label">
            成熟度状态
            <Choice
              label="成熟度状态"
              value={draft.maturity}
              options={[
                { value: 'showcase', label: '已验证样板（优先展示）' },
                { value: 'draft', label: '待打磨草稿（平时收起）' },
              ]}
              onChange={v => setDraft({ ...draft, maturity: v as Maturity })}
              disabled={busy}
            />
          </label>
        </div>

        <div className="field-label">
          样板间成品图／参考图（在画廊封面直观展示）
          <div className="edit-images">
            {draft.images.map((name, i) => (
              <div className="edit-image" key={`${name}-${i}`}>
                <img src={imageUrl(directory, name)} alt={`样板图 ${i + 1}`} />
                <div className="image-actions">
                  <button disabled={busy || i === 0} onClick={() => setDraft(old => ({ ...old, images: [name, ...old.images.filter((_, j) => j !== i)] }))}>{i === 0 ? '封面样板' : '设为封面'}</button>
                  <button disabled={busy} onClick={() => setDraft(old => ({ ...old, images: old.images.filter((_, j) => j !== i) }))}>移除</button>
                </div>
              </div>
            ))}
            <button className="add-image" disabled={busy} onClick={() => imageFiles.current?.click()}><PlusIcon /><span>添加样板图</span></button>
          </div>
          <input ref={imageFiles} type="file" hidden multiple accept="image/png,image/jpeg,image/webp" onChange={e => { void addImages(e.target.files); }} />
        </div>

        <div className="editor-two-grid">
          <label className="field-label">
            输入示例／适用场景（可选）
            <textarea className="instructions-editor compact-textarea" value={draft.sampleInput} onChange={e => setDraft({ ...draft, sampleInput: e.target.value })} placeholder="例如：把『什么是零拷贝』拆成小红书图文" disabled={busy} />
          </label>
          <label className="field-label">
            文字样板输出／版式效果说明（无图片时作为封面展示）
            <textarea className="instructions-editor compact-textarea" value={draft.sampleOutput} onChange={e => setDraft({ ...draft, sampleOutput: e.target.value })} placeholder="贴一段代表性输出成稿片段，或简述这套样板的视觉与平台规格" disabled={busy} />
          </label>
        </div>

        {!isPrompt && (
          <div className="folder-import">
            <button className="text-button primary-action" disabled={busy} onClick={() => folderInput.current?.click()}>整夹导入 Skill</button>
            <span>{imported ? `已读取 ${imported.length} 个文件待保存` : '根目录包含 SKILL.md，连同参考资料与脚本按原结构保存'}</span>
            <input ref={folderInput} type="file" hidden multiple {...{ webkitdirectory: '' }} onChange={e => { void importFolder(e.target.files); }} />
          </div>
        )}
        {imported && <ul className="skill-file-tree import-tree">{imported.map(f => <li key={f.path}>{f.path}</li>)}</ul>}

        <label className="field-label" htmlFor="asset-rules">{isPrompt ? '底层 Prompt 正文' : '底层 SKILL.md 文档'}</label>
        <textarea
          id="asset-rules"
          className="instructions-editor"
          value={isPrompt ? draft.prompt : draft.skillMarkdown}
          onChange={e => setDraft({ ...draft, [isPrompt ? 'prompt' : 'skillMarkdown']: e.target.value })}
          spellCheck={false}
          disabled={busy}
          placeholder={isPrompt ? '填写可复用的提示词正文' : '编写 SKILL.md，或点击上方整夹导入'}
        />

        <label className="field-label notes-field">
          使用备注（可选）
          <input value={draft.notes} maxLength={4000} onChange={e => setDraft({ ...draft, notes: e.target.value })} placeholder="记录这套样板最近在哪类选题上效果最好、有哪些注意点" disabled={busy} />
        </label>

        {error && <p className="inline-error" role="alert">{error}</p>}
      </div>
    </section>
  );
}
