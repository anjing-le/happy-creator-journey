import { useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { StudioEntrance, StudioLaunch } from './navigation';
import {
  AssetEntry,
  AssetKind,
  AssetLibrary,
  MODALITY_LABELS,
  Modality,
  SkillFile,
  deleteEntry,
  emptyEntry,
  imageUrl,
  importImage,
  loadLibrary,
  openSkillFolder,
  readSkillFolder,
  saveEntry,
  skillFiles,
} from './library';

const native = isTauri();

function BackIcon() {
  return <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m14 6-6 6 6 6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>;
}
function PlusIcon() {
  return <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 5v14M5 12h14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>;
}
function errText(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}

function launchTarget(entry: AssetEntry): { entrance: StudioEntrance; launch: StudioLaunch } {
  return {
    entrance: entry.modality === 'video' ? '视频' : '图文',
    launch: {
      skillId: entry.kind === 'skill' ? entry.id : undefined,
      promptId: entry.kind === 'prompt' ? entry.id : undefined,
      nonce: Date.now(),
    },
  };
}

export function Showcase({ back, onLaunch }: { back: () => void; onLaunch: (entrance: StudioEntrance, launch: StudioLaunch) => void }) {
  const [library, setLibrary] = useState<AssetLibrary>({ entries: [], imageDirectory: '', tags: [] });
  const [builtins, setBuiltins] = useState<AssetEntry[]>([]);
  const [kindFilter, setKindFilter] = useState<'all' | AssetKind>('all');
  const [modalityFilter, setModalityFilter] = useState<'all' | Modality>('all');
  const [tagFilter, setTagFilter] = useState<string>('');
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<AssetEntry>();
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const editingRef = useRef(false);
  editingRef.current = editing;

  async function refresh() {
    if (editingRef.current) return;
    try {
      const [lib, built] = await Promise.all([
        loadLibrary(),
        native ? invoke<AssetEntry[]>('creation_builtin_skills') : Promise.resolve([]),
      ]);
      setLibrary(lib);
      setBuiltins(built);
      setSelected(old => old ? [...built, ...lib.entries].find(item => item.id === old.id) : undefined);
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

  const allEntries = [...builtins, ...library.entries];
  const allTags = [...new Set([...library.tags, ...builtins.flatMap(b => b.tags)])].sort();

  if (selected && editing) {
    return (
      <ShowcaseEditor
        entry={selected}
        directory={library.imageDirectory}
        knownTags={allTags}
        cancel={() => {
          setEditing(false);
          if (!selected.id) setSelected(undefined);
        }}
        saved={async saved => {
          editingRef.current = false;
          setEditing(false);
          await refresh();
          setSelected(saved);
        }}
      />
    );
  }

  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const filtered = allEntries.filter(entry => {
    if (kindFilter !== 'all' && entry.kind !== kindFilter) return false;
    if (modalityFilter !== 'all' && entry.modality !== modalityFilter) return false;
    if (tagFilter && !entry.tags.includes(tagFilter)) return false;
    if (!terms.length) return true;
    const hay = `${entry.name} ${entry.sample} ${entry.prompt} ${entry.skillMarkdown} ${entry.notes} ${entry.tags.join(' ')}`.toLocaleLowerCase();
    return terms.every(t => hay.includes(t));
  });

  function startCreate(kind: AssetKind) {
    const draft = emptyEntry(kind, modalityFilter === 'all' ? (kind === 'skill' ? 'image' : 'text') : modalityFilter);
    if (tagFilter) draft.tags = [tagFilter];
    setSelected(draft);
    setEditing(true);
  }

  return (
    <section className="page showcase-page" aria-label="样板间">
      <header className="page-header showcase-header">
        <button className="icon-button" onClick={back} aria-label="返回首页"><BackIcon /></button>
        <nav className="library-tabs compact-tabs" aria-label="载体筛选">
          <button className={kindFilter === 'all' ? 'active' : ''} onClick={() => { setKindFilter('all'); setSelected(undefined); }}>
            全部<span className="tab-count">{allEntries.length}</span>
          </button>
          <button className={kindFilter === 'prompt' ? 'active' : ''} onClick={() => { setKindFilter('prompt'); setSelected(undefined); }}>
            提示词<span className="tab-count">{library.entries.filter(e => e.kind === 'prompt').length}</span>
          </button>
          <button className={kindFilter === 'skill' ? 'active' : ''} onClick={() => { setKindFilter('skill'); setSelected(undefined); }}>
            Skills<span className="tab-count">{allEntries.filter(e => e.kind === 'skill').length}</span>
          </button>
        </nav>
        <span className="header-space" />
        <div className="showcase-toolbar-actions">
          <div className="segmented-pills" role="group" aria-label="原子能力方向">
            {(['all', 'text', 'image', 'video'] as const).map(m => (
              <button key={m} className={modalityFilter === m ? 'active' : ''} onClick={() => { setModalityFilter(m); setSelected(undefined); }}>
                {m === 'all' ? '全部' : MODALITY_LABELS[m]}
              </button>
            ))}
          </div>
          <input
            className="showcase-search"
            aria-label="搜索样板或标签"
            placeholder="搜名称、正文或 #标签…"
            value={query}
            onChange={e => setQuery(e.target.value)}
          />
          <button className="text-button primary-action" onClick={() => startCreate('prompt')}>＋ 提示词</button>
          <button className="text-button" onClick={() => startCreate('skill')}>＋ Skill</button>
        </div>
      </header>

      {error && <div className="browser-error inline-error" role="alert">{error}</div>}

      {selected ? (
        <ShowcaseDetail
          entry={selected}
          directory={library.imageDirectory}
          close={() => setSelected(undefined)}
          edit={() => setEditing(true)}
          pickTag={tag => { setTagFilter(tag); setSelected(undefined); }}
          useInStudio={() => {
            const target = launchTarget(selected);
            onLaunch(target.entrance, target.launch);
          }}
          deleted={async () => {
            setSelected(undefined);
            await refresh();
          }}
        />
      ) : (
        <div className="showcase-body">
          {allTags.length > 0 && (
            <div className="showcase-subfilters" aria-label="标签筛选">
              <button className={`filter-chip ${!tagFilter ? 'active' : ''}`} onClick={() => setTagFilter('')}>全部标签</button>
              {allTags.map(tag => (
                <button
                  key={tag}
                  className={`filter-chip ${tagFilter === tag ? 'active' : ''}`}
                  onClick={() => setTagFilter(tagFilter === tag ? '' : tag)}
                >
                  #{tag}
                </button>
              ))}
            </div>
          )}

          {loading ? (
            <div className="empty-library"><p>正在读取样板间…</p></div>
          ) : filtered.length === 0 ? (
            <div className="empty-library">
              <button className="empty-add" onClick={() => startCreate(kindFilter === 'skill' ? 'skill' : 'prompt')}>
                <PlusIcon />
                <span>当前还没有匹配的样板，点击沉淀第一条细粒度提示词或 Skill</span>
              </button>
            </div>
          ) : (
            <div className="showcase-grid">
              {filtered.map(entry => {
                const isBuiltin = entry.id.startsWith('builtin:');
                const previewText = entry.sample.trim() || entry.notes.trim() || (entry.kind === 'prompt' ? entry.prompt : entry.skillMarkdown);
                return (
                  <article key={entry.id} className="showcase-card">
                    <button className="showcase-card-cover" onClick={() => setSelected(entry)} aria-label={`查看样板 ${entry.name}`}>
                      {entry.images.length > 0 ? (
                        <div className="showcase-cover-art">
                          {entry.images.length > 1 && <span className="series-sheet" aria-hidden="true" />}
                          <img src={imageUrl(library.imageDirectory, entry.images[0])} alt={entry.name} loading="lazy" />
                          {entry.images.length > 1 && <span className="image-count-badge">{entry.images.length} 张</span>}
                        </div>
                      ) : (
                        <div className="showcase-cover-text">
                          <span className="sample-kicker">{entry.sample.trim() ? '样板效果' : entry.kind === 'skill' ? 'Skill 摘要' : 'Prompt 摘要'}</span>
                          <p>{previewText.slice(0, 140)}</p>
                        </div>
                      )}
                    </button>
                    <div className="showcase-card-meta">
                      <div className="showcase-badges">
                        <span className="badge format">{MODALITY_LABELS[entry.modality]}</span>
                        <span className="badge">{entry.kind === 'skill' ? 'Skill' : '提示词'}</span>
                        {isBuiltin && <span className="badge builtin">内置</span>}
                        {entry.tags.map(tag => (
                          <button key={tag} className="badge tag-btn" onClick={() => setTagFilter(tag)}>#{tag}</button>
                        ))}
                      </div>
                      <div className="showcase-card-footer">
                        <button className="showcase-title-btn" onClick={() => setSelected(entry)}>{entry.name}</button>
                        <button
                          className="text-button use-template-btn"
                          onClick={() => {
                            const target = launchTarget(entry);
                            onLaunch(target.entrance, target.launch);
                          }}
                        >
                          引用 →
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

function ShowcaseDetail({
  entry, directory, close, edit, pickTag, useInStudio, deleted,
}: {
  entry: AssetEntry;
  directory: string;
  close: () => void;
  edit: () => void;
  pickTag: (tag: string) => void;
  useInStudio: () => void;
  deleted: () => Promise<void>;
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

  async function copyBody() {
    try {
      await navigator.clipboard.writeText(isPrompt ? entry.prompt : entry.skillMarkdown);
      setCopied(true);
    } catch { setError('复制失败，请手动选中复制'); }
  }

  return (
    <div className="showcase-body">
      <article className="showcase-detail-panel" aria-label={entry.name}>
        <header className="material-heading">
          <button className="icon-button" onClick={close} aria-label="返回样板列表"><BackIcon /></button>
          <div>
            <div className="showcase-badges">
              <span className="badge format">{MODALITY_LABELS[entry.modality]}</span>
              <span className="badge">{isPrompt ? '提示词' : 'Skill'}</span>
              {isBuiltin && <span className="badge builtin">内置</span>}
              {entry.tags.map(t => <button key={t} className="badge tag-btn" onClick={() => pickTag(t)}>#{t}</button>)}
            </div>
            <h2>{entry.name}</h2>
          </div>
          <span className="header-space" />
          <button className="text-button primary-action" onClick={useInStudio}>在创作中引用 →</button>
          <button className="text-button" onClick={() => { void copyBody(); }}>{copied ? '已复制' : '复制正文'}</button>
          {!isPrompt && !isBuiltin && <button className="text-button" onClick={() => { void openSkillFolder(entry.id).catch(e => setError(errText(e))); }}>打开文件夹</button>}
          {!isBuiltin && <button className="text-button" onClick={edit}>编辑</button>}
          {!isBuiltin && (confirmDelete ? (
            <button className="text-button danger-button" onClick={() => { void deleteEntry(entry).then(deleted).catch(e => setError(errText(e))); }}>确认删除</button>
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

        {entry.sample.trim() && (
          <div className="sample-box">
            <span className="sample-kicker">样板效果输出</span>
            <p>{entry.sample}</p>
          </div>
        )}

        {entry.notes && <p className="material-notes">{entry.notes}</p>}

        {!isPrompt && (
          <details className="skill-package" open>
            <summary>Skill 文件夹 · {files.length} 个文件</summary>
            <ul className="skill-file-tree">{files.map(path => <li key={path}>{path}</li>)}</ul>
          </details>
        )}

        <details className="rules-section" open={entry.images.length === 0 && !entry.sample.trim()}>
          <summary className="rules-toggle">{isPrompt ? 'Prompt 正文' : 'SKILL.md 正文'}</summary>
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

export function TagPicker({
  tags, knownTags, onChange, disabled = false,
}: {
  tags: string[]; knownTags: string[]; onChange: (next: string[]) => void; disabled?: boolean;
}) {
  const [input, setInput] = useState('');

  function addTag(raw: string) {
    const clean = raw.trim().replace(/^#+/, '').trim();
    if (!clean || tags.includes(clean) || tags.length >= 24) return;
    onChange([...tags, clean]);
  }

  function commitInput() {
    for (const part of input.split(/[,，\s]+/)) addTag(part);
    setInput('');
  }

  const unusedKnown = knownTags.filter(t => !tags.includes(t));

  return (
    <div className="tag-picker">
      <div className="tag-selected-row">
        {tags.map(t => (
          <span key={t} className="tag-pill">
            #{t}
            {!disabled && <button type="button" aria-label={`移除标签 ${t}`} onClick={() => onChange(tags.filter(x => x !== t))}>×</button>}
          </span>
        ))}
        <input
          value={input}
          disabled={disabled}
          maxLength={32}
          placeholder={tags.length ? '继续加标签…' : '输入标签按回车（如：安静、计算机、蜡笔风）'}
          onChange={e => setInput(e.target.value)}
          onKeyDown={e => {
            if ((e.key === 'Enter' || e.key === ',') && !e.nativeEvent.isComposing) {
              e.preventDefault();
              commitInput();
            } else if (e.key === 'Backspace' && !input && tags.length) {
              onChange(tags.slice(0, -1));
            }
          }}
          onBlur={() => { if (input.trim()) commitInput(); }}
        />
      </div>
      {unusedKnown.length > 0 && (
        <div className="tag-suggestions">
          <span>点选已有标签：</span>
          {unusedKnown.map(t => (
            <button key={t} type="button" disabled={disabled} className="filter-chip" onClick={() => addTag(t)}>
              #{t}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function ShowcaseEditor({
  entry, directory, knownTags, cancel, saved,
}: {
  entry: AssetEntry; directory: string; knownTags: string[]; cancel: () => void; saved: (entry: AssetEntry) => Promise<void>;
}) {
  const [draft, setDraft] = useState<AssetEntry>({ ...entry, images: [...entry.images], tags: [...entry.tags] });
  const [imported, setImported] = useState<SkillFile[]>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const imageFiles = useRef<HTMLInputElement>(null);
  const folderInput = useRef<HTMLInputElement>(null);
  const isPrompt = draft.kind === 'prompt';

  async function addImages(selected: FileList | null) {
    if (!selected?.length || busy) return;
    if (draft.images.length + selected.length > 16) { setError('每份样板最多保存 16 张图片'); return; }
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
    <section className="page" data-editing aria-label="编辑样板">
      <header className="page-header">
        <button className="icon-button" onClick={cancel} disabled={busy} aria-label="取消编辑"><BackIcon /></button>
        <h1>{entry.id ? '编辑样板' : isPrompt ? '添加提示词样板' : '添加 Skill 样板'}</h1>
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
        <div className="editor-top-row">
          <label className="field-label grow-field">
            名称
            <input autoFocus maxLength={80} value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} placeholder={isPrompt ? '例如：安静粗蜡笔角色设定 / 技术拆解口吻' : '例如：单张卡片生图'} disabled={busy} />
          </label>
          <div className="field-label">
            原子方向
            <div className="segmented-pills modality-pills" role="group" aria-label="选择原子能力方向">
              {(['text', 'image', 'video'] as const).map(m => (
                <button key={m} type="button" className={draft.modality === m ? 'active' : ''} disabled={busy} onClick={() => setDraft({ ...draft, modality: m })}>
                  {MODALITY_LABELS[m]}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="field-label">
          Tags（用标签区分角色、画风或领域，如 #安静 #计算机 #金融）
          <TagPicker tags={draft.tags} knownTags={knownTags} onChange={tags => setDraft({ ...draft, tags })} disabled={busy} />
        </div>

        <div className="field-label">
          样板间展示图／参考图（可选，既是直观样板，引用时也自动作为参考图）
          <div className="edit-images">
            {draft.images.map((name, i) => (
              <div className="edit-image" key={`${name}-${i}`}>
                <img src={imageUrl(directory, name)} alt={`样板图 ${i + 1}`} />
                <div className="image-actions">
                  <button disabled={busy || i === 0} onClick={() => setDraft(old => ({ ...old, images: [name, ...old.images.filter((_, j) => j !== i)] }))}>{i === 0 ? '封面' : '设为封面'}</button>
                  <button disabled={busy} onClick={() => setDraft(old => ({ ...old, images: old.images.filter((_, j) => j !== i) }))}>移除</button>
                </div>
              </div>
            ))}
            <button className="add-image" disabled={busy} onClick={() => imageFiles.current?.click()}><PlusIcon /><span>添加图片</span></button>
          </div>
          <input ref={imageFiles} type="file" hidden multiple accept="image/png,image/jpeg,image/webp" onChange={e => { void addImages(e.target.files); }} />
        </div>

        {!isPrompt && (
          <div className="folder-import">
            <button className="text-button primary-action" disabled={busy} onClick={() => folderInput.current?.click()}>导入 Skill 文件夹</button>
            <span>{imported ? `已读取 ${imported.length} 个文件待保存` : '包含 SKILL.md、参考资料或脚本的精简文件夹'}</span>
            <input ref={folderInput} type="file" hidden multiple {...{ webkitdirectory: '' }} onChange={e => { void importFolder(e.target.files); }} />
          </div>
        )}
        {imported && <ul className="skill-file-tree import-tree">{imported.map(f => <li key={f.path}>{f.path}</li>)}</ul>}

        <label className="field-label" htmlFor="asset-rules">{isPrompt ? '提示词正文' : 'SKILL.md'}</label>
        <textarea
          id="asset-rules"
          className="instructions-editor"
          value={isPrompt ? draft.prompt : draft.skillMarkdown}
          onChange={e => setDraft({ ...draft, [isPrompt ? 'prompt' : 'skillMarkdown']: e.target.value })}
          spellCheck={false}
          disabled={busy}
          placeholder={isPrompt ? '填写精简明确的生文、生图或生视频 Prompt' : '编写精简 SKILL.md，或导入已有文件夹'}
        />

        <label className="field-label">
          文字样板示例（无样板图时直接显示在样板间卡片上，可选）
          <textarea className="instructions-editor compact-textarea" value={draft.sample} onChange={e => setDraft({ ...draft, sample: e.target.value })} placeholder="贴一段这套提示词产出的代表性文字或分镜片段" disabled={busy} />
        </label>

        <label className="field-label">
          备注（可选）
          <input value={draft.notes} maxLength={4000} onChange={e => setDraft({ ...draft, notes: e.target.value })} placeholder="简短记录适用场景或替换变量" disabled={busy} />
        </label>

        {error && <p className="inline-error" role="alert">{error}</p>}
      </div>
    </section>
  );
}
