import { useEffect, useRef, useState } from 'react';
import { Creation } from './Creation';
import { isTauri } from '@tauri-apps/api/core';
import { AssetLibrary, AssetEntry, AssetKind, emptyEntry, imageUrl, importImage, loadLibrary, saveEntry, SkillFile, readSkillFolder, skillFiles, openSkillFolder, addCategory, moveCategory } from './library';

type Route = { page: 'home' } | { page: 'library' } | { page: 'creation'; name: string };
const HOME: Route = { page: 'home' };
const native = isTauri();
const entrances = [
  { name: '文', image: 'writing' },
  { name: '图片', image: 'images' },
  { name: '视频', image: 'video' },
];
function BackIcon() { return <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m14 6-6 6 6 6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>; }
function PlusIcon() { return <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 5v14M5 12h14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>; }
function message(error: unknown) { return error instanceof Error ? error.message : String(error); }

export function Workspace() {
  const [history, setHistory] = useState<Route[]>([HOME]);
  const [position, setPosition] = useState(0);
  const route = history[position];
  const navigate = (next: Route) => { setHistory(old => [...old.slice(0, position + 1), next]); setPosition(position + 1); };
  const back = () => setPosition(old => Math.max(0, old - 1));
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (document.querySelector('[role="dialog"], [data-editing]') || event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      if ((event.metaKey || event.ctrlKey) && event.key === '1') { event.preventDefault(); navigate(HOME); }
      else if ((event.metaKey && event.key === '[') || (event.altKey && event.key === 'ArrowLeft')) { event.preventDefault(); back(); }
      else if ((event.metaKey && event.key === ']') || (event.altKey && event.key === 'ArrowRight')) { event.preventDefault(); setPosition(old => Math.min(history.length - 1, old + 1)); }
    };
    const mouse = (event: MouseEvent) => {
      if (document.querySelector('[role="dialog"], [data-editing]')) return;
      if (event.button === 3 || event.button === 4) { event.preventDefault(); setPosition(old => Math.max(0, Math.min(history.length - 1, old + (event.button === 3 ? -1 : 1)))); }
    };
    window.addEventListener('keydown', key); window.addEventListener('mouseup', mouse);
    return () => { window.removeEventListener('keydown', key); window.removeEventListener('mouseup', mouse); };
  }, [position, history]);
  if (route.page === 'home') return <section className="home" aria-label="创作首页">
    <div className="home-entries">
      <div className="creation-entries">{entrances.map(({ name, image }) => <button key={name} className={`entry-card entrance-${image}`} aria-label={name} onClick={() => navigate({ page: 'creation', name })}>
        <span className="entrance-art"><img src={`/entrances/${image}-v2.png`} alt="" draggable={false} /></span>
      </button>)}</div>
      <button className="library-entry" aria-label="素材库" onClick={() => navigate({ page: 'library' })}>
        <img src="/entrances/library-v2.png" alt="" draggable={false} />
      </button>
    </div>
  </section>;
  if (route.page === 'library') return <LibraryView back={back} />;
  return <Creation key={route.name} name={route.name} back={back} />;
}

function LibraryView({ back }: { back: () => void }) {
  const [library, setLibrary] = useState<AssetLibrary>({ entries: [], imageDirectory: '', categories: [] });
  const [kind, setKind] = useState<AssetKind>('prompt');
  const [selected, setSelected] = useState<AssetEntry>();
  const [editing, setEditing] = useState(false);
  const [newCategory, setNewCategory] = useState('');
  const [addingCategory, setAddingCategory] = useState(false);
  const [dropCategory, setDropCategory] = useState<string | null>(null);
  const [moving, setMoving] = useState(false);
  const drag = useRef<{ id: string; x: number; y: number; active: boolean; target: string | null } | null>(null);
  const suppressClick = useRef(false);
  const [draggingId, setDraggingId] = useState('');
  const [query, setQuery] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const [collapsed, setCollapsed] = useState<string[]>(() => {
    try { const value: unknown = JSON.parse(localStorage.getItem('creatorx.library.collapsed') || '[]'); return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : []; } catch { return []; }
  });
  const editingRef = useRef(false); editingRef.current = editing;
  async function refresh() {
    if (editingRef.current) return;
    try { const result = await loadLibrary(); setLibrary(result); setSelected(old => old ? result.entries.find(entry => entry.id === old.id) : undefined); setError(''); }
    catch (error) { setError(message(error)); } finally { setLoading(false); }
  }
  useEffect(() => { void refresh(); const focus = () => { void refresh(); }; window.addEventListener('focus', focus); return () => window.removeEventListener('focus', focus); }, []);
  function toggle(category: string) { setCollapsed(old => { const next = old.includes(category) ? old.filter(c => c !== category) : [...old, category]; localStorage.setItem('creatorx.library.collapsed', JSON.stringify(next)); return next; }); }
  const create = () => { setSelected(emptyEntry(kind)); setEditing(true); };
  async function saved(entry: AssetEntry) { editingRef.current = false; setEditing(false); await refresh(); setSelected(entry); }
  if (selected && editing) return <AssetEditor entry={selected} directory={library.imageDirectory} categories={library.categories} cancel={() => { setEditing(false); if (!selected.id) setSelected(undefined); }} saved={saved} />;
  const entries = library.entries.filter(entry => entry.kind === kind);
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const matching = entries.filter(entry => terms.every(term => `${entry.name} ${entry.prompt} ${entry.skillMarkdown} ${entry.notes} ${entry.category}`.toLocaleLowerCase().includes(term)));
  const categories = [...new Set([...library.categories, ...entries.map(e => e.category)].filter(Boolean)), ''];
  async function createCategory() { if (!newCategory.trim()) return; try { await addCategory(newCategory); setNewCategory(''); setAddingCategory(false); await refresh(); } catch (e) { setError(message(e)); } }
  async function drop(id: string, category: string) { setDropCategory(null); if (moving) return; const entry = library.entries.find(e => e.id === id); if (!entry || entry.category === category) return; setMoving(true); try { const updated = await moveCategory(entry, category); await refresh(); setSelected(updated); } catch (e) { setError(message(e)); } finally { setMoving(false); } }
  return <section className="page" aria-label="素材库">
    <header className="page-header library-header"><button className="icon-button" onClick={back} aria-label="返回首页"><BackIcon /></button><nav className="library-tabs compact-tabs" aria-label="素材类型">{([['prompt', '提示词'], ['skill', 'Skills']] as const).map(([value, label]) => <button key={value} className={kind === value ? 'active' : ''} onClick={() => { setKind(value); setSelected(undefined); setQuery(''); }}>{label}<span className="tab-count">{library.entries.filter(e => e.kind === value).length}</span></button>)}</nav></header>
    {error && <div className="browser-error inline-error" role="alert">{error}</div>}
    <div className="library-browser">
      <aside className="library-sidebar"><div className="library-search-row"><input className="library-search" aria-label="搜索素材" placeholder="搜索素材" value={query} onChange={e => setQuery(e.target.value)} /><div className="library-add"><button className="icon-button" aria-label="新增素材或分类" aria-expanded={addingCategory} onClick={() => setAddingCategory(!addingCategory)}><PlusIcon /></button>{addingCategory && <div className="library-add-menu"><button onClick={() => { setAddingCategory(false); create(); }}>{kind === 'prompt' ? '新增提示词' : '导入 Skill'}</button><form className="category-create" onSubmit={e => { e.preventDefault(); void createCategory(); }}><input autoFocus aria-label="新分类名称" maxLength={80} value={newCategory} onChange={e => setNewCategory(e.target.value)} placeholder="新分类名称" /><button disabled={!newCategory.trim()} aria-label="保存新分类">添加</button></form></div>}</div></div>
        {loading ? <p className="sidebar-note">正在打开…</p> : matching.length === 0 && query ? <p className="sidebar-note">没有找到匹配内容</p> : categories.map(category => {
          const group = matching.filter(e => (e.category || '') === category);
          const isClosed = !query && collapsed.includes(category);
          return <div className={`prompt-group ${dropCategory === category ? 'drop-target' : ''}`} key={category} data-category={category} onDragOver={e => { if (e.dataTransfer.types.includes('application/x-creatorx-material')) { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; setDropCategory(category); } }} onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDropCategory(null); }} onDrop={e => { e.preventDefault(); void drop(e.dataTransfer.getData('application/x-creatorx-material'), category); }}><button className="group-heading" aria-expanded={!isClosed} onClick={() => toggle(category)}><span className={isClosed ? 'tree-chevron closed' : 'tree-chevron'}>⌄</span><span>{category || '未分类'}</span><span className="group-count">{group.length}</span></button>{!isClosed && group.map(entry => <button key={entry.id} className={`prompt-row ${selected?.id === entry.id ? 'selected' : ''} ${draggingId === entry.id ? 'dragging' : ''}`} draggable={false} onPointerDown={e => { if (moving || e.button !== 0) return; drag.current = { id: entry.id, x: e.clientX, y: e.clientY, active: false, target: null }; e.currentTarget.setPointerCapture(e.pointerId); }} onPointerMove={e => { const current = drag.current; if (!current || current.id !== entry.id) return; if (!current.active && Math.hypot(e.clientX - current.x, e.clientY - current.y) > 6) { current.active = true; setDraggingId(entry.id); } if (!current.active) return; e.preventDefault(); const target = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('[data-category]'); current.target = target?.dataset.category ?? null; setDropCategory(current.target); }} onPointerUp={e => { const current = drag.current; drag.current = null; setDraggingId(''); setDropCategory(null); if (current?.active) { e.preventDefault(); suppressClick.current = true; window.setTimeout(() => { suppressClick.current = false; }, 0); if (current.target !== null) void drop(current.id, current.target); } }} onPointerCancel={() => { drag.current = null; setDraggingId(''); setDropCategory(null); }} onClick={() => { if (!suppressClick.current) setSelected(entry); }} title={entry.name}><span>{entry.name}</span></button>)}{!isClosed && !group.length && <button className="prompt-row" onClick={() => { setSelected({ ...emptyEntry(kind), category }); setEditing(true); }}>添加到此分类</button>}</div>;
        })}
      </aside>
      <div className="library-reader">{selected ? <AssetDetail key={selected.id + ':' + selected.revision} entry={selected} directory={library.imageDirectory} edit={() => setEditing(true)} /> : <div className="reader-empty"><button className="text-button" onClick={entries.length ? () => setSelected(matching[0] || entries[0]) : create}>{entries.length ? '选择一项，查看与使用' : kind === 'prompt' ? '从一条提示词开始' : '从一个 Skill 文件夹开始'}</button></div>}</div>
    </div>
  </section>;
}

function AssetDetail({ entry, directory, edit }: { entry: AssetEntry; directory: string; edit: () => void }) {
  const [index, setIndex] = useState(0);
  const [enlarged, setEnlarged] = useState(false);
  const [files, setFiles] = useState<string[]>([]);
  const [error, setError] = useState('');
  const [copied, setCopied] = useState(false);
  const zoom = useRef<HTMLDialogElement>(null);
  const isPrompt = entry.kind === 'prompt';
  useEffect(() => { if (enlarged) zoom.current?.showModal(); else zoom.current?.close(); }, [enlarged]);
  useEffect(() => { if (!isPrompt) void skillFiles(entry.id).then(setFiles).catch(e => setError(message(e))); }, [entry.id, isPrompt]);
  async function copy() { try { await navigator.clipboard.writeText(entry.prompt); setCopied(true); } catch { setError('复制失败，请选择正文后复制'); } }
  return <article className="material-detail" aria-label={entry.name}>
    <header className="material-heading"><div><p className="material-category">{entry.category || '未分类'}</p><h2>{entry.name}</h2></div><span className="header-space" />{isPrompt ? <button className="text-button primary-copy" onClick={() => { void copy(); }}>{copied ? '已复制' : '复制提示词'}</button> : <button className="text-button" onClick={() => { void openSkillFolder(entry.id).catch(e => setError(message(e))); }}>打开文件夹</button>}<button className="text-button" onClick={edit}>编辑</button></header>
    {entry.notes && <p className="material-notes">{entry.notes}</p>}
    {!isPrompt && <details className="skill-package" open><summary>Skill 文件夹 · {files.length} 个文件</summary>{files.length ? <ul className="skill-file-tree">{files.map(path => <li key={path} style={{ paddingLeft: `${Math.min(path.split('/').length - 1, 6) * 12}px` }}>{path}</li>)}</ul> : <p>保存后建立文件夹，或导入完整 Skill。</p>}</details>}
    <pre className="prompt-body">{isPrompt ? entry.prompt : entry.skillMarkdown}</pre>
    {entry.images.length > 0 && <details className="reference-images" open><summary>参考图 · {entry.images.length}</summary><button className="reader-art" aria-label="放大参考图" onClick={() => setEnlarged(true)}><img src={imageUrl(directory, entry.images[index])} alt={entry.name} /></button>{entry.images.length > 1 && <div className="filmstrip">{entry.images.map((name, i) => <button key={`${name}-${i}`} aria-label={`查看第 ${i + 1} 张`} aria-pressed={i === index} onClick={() => setIndex(i)}><img src={imageUrl(directory, name)} alt={`参考图 ${i + 1}`} /></button>)}</div>}</details>}
    {entry.source && <p className="asset-source">来源：{entry.source}</p>}
    {(entry.originCreatedAt || entry.originUpdatedAt) && <p className="asset-source">原始创建：{entry.originCreatedAt || '—'} · 原始更新：{entry.originUpdatedAt || '—'}</p>}
    {error && <p className="inline-error" role="alert">{error}</p>}
    <dialog ref={zoom} className="image-lightbox" onCancel={e => { e.preventDefault(); setEnlarged(false); }} onClick={e => { if (e.target === e.currentTarget) setEnlarged(false); }}><button className="lightbox-close icon-button" onClick={() => setEnlarged(false)} aria-label="关闭大图">×</button>{entry.images[index] && <img src={imageUrl(directory, entry.images[index])} alt={entry.name} />}</dialog>
  </article>;
}

function AssetEditor({ entry, directory, categories, cancel, saved }: { entry: AssetEntry; directory: string; categories: string[]; cancel: () => void; saved: (entry: AssetEntry) => Promise<void> }) {
  const [draft, setDraft] = useState<AssetEntry>({ ...entry, images: [...entry.images] });
  const [imported, setImported] = useState<SkillFile[]>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const imageFiles = useRef<HTMLInputElement>(null);
  const folder = useRef<HTMLInputElement>(null);
  const isPrompt = draft.kind === 'prompt';
  async function addImages(selected: FileList | null) {
    if (!selected?.length || busy) return;
    if (draft.images.length + selected.length > 16) { setError('每份素材最多保存 16 张参考图'); return; }
    setBusy(true); setError('');
    try { const names: string[] = []; for (const file of Array.from(selected)) names.push(await importImage(file)); setDraft(old => ({ ...old, images: [...old.images, ...names] })); }
    catch (error) { setError(message(error)); } finally { setBusy(false); if (imageFiles.current) imageFiles.current.value = ''; }
  }
  async function importFolder(selected: FileList | null) {
    if (!selected?.length || busy) return;
    setBusy(true); setError('');
    try { const result = await readSkillFolder(selected); setImported(result.files); setDraft(old => ({ ...old, name: old.name || result.name, skillMarkdown: result.markdown })); }
    catch (error) { setError(message(error)); } finally { setBusy(false); if (folder.current) folder.current.value = ''; }
  }
  async function save() {
    if (busy) return;
    if (!native) { setError('请在 CreatorX 客户端中保存素材'); return; }
    setBusy(true); setError('');
    try { await saved(await saveEntry(draft, imported)); }
    catch (error) { setError(message(error)); } finally { setBusy(false); }
  }
  return <section className="page" data-editing aria-label="编辑素材"><header className="page-header"><button className="icon-button" onClick={cancel} disabled={busy} aria-label="取消编辑"><BackIcon /></button><h1>{entry.id ? '编辑' : isPrompt ? '添加提示词' : '添加 Skill'}</h1><span className="header-space" /><button className="text-button save-button" onClick={() => { void save(); }} disabled={busy || !draft.name.trim() || !(isPrompt ? draft.prompt : draft.skillMarkdown).trim()}>{busy ? '处理中…' : '保存'}</button></header>
    <div className="asset-editor"><label className="field-label">名称<input autoFocus maxLength={80} value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} placeholder={isPrompt ? '这条提示词用来做什么' : 'Skill 的名字'} disabled={busy} /></label>
      <div className="editor-columns"><label className="field-label">分类<input list="material-categories" maxLength={80} placeholder="未分类" value={draft.category} onChange={e => setDraft({ ...draft, category: e.target.value })} disabled={busy} /></label></div><datalist id="material-categories">{categories.map(category => <option key={category} value={category} />)}</datalist>
      {!isPrompt && <div className="folder-import"><button className="text-button" disabled={busy} onClick={() => folder.current?.click()}>导入 Skill 文件夹</button><span>{imported ? `${imported.length} 个文件待保存` : 'SKILL.md、脚本、参考资料和图片一起保留'}</span><input ref={folder} type="file" hidden multiple {...{ webkitdirectory: '' }} onChange={e => { void importFolder(e.target.files); }} /></div>}
      {imported && <ul className="skill-file-tree import-tree">{imported.map(file => <li key={file.path}>{file.path}</li>)}</ul>}
      <label className="field-label" htmlFor="asset-rules">{isPrompt ? '提示词' : 'SKILL.md'}</label><textarea id="asset-rules" className="instructions-editor" value={isPrompt ? draft.prompt : draft.skillMarkdown} onChange={e => setDraft({ ...draft, [isPrompt ? 'prompt' : 'skillMarkdown']: e.target.value })} spellCheck={false} disabled={busy} placeholder={isPrompt ? '可以反复使用的一段 Prompt' : '在这里编写 SKILL.md，或导入已有文件夹'} />
      <label className="field-label notes-field">备注<input value={draft.notes} maxLength={4000} onChange={e => setDraft({ ...draft, notes: e.target.value })} placeholder="何时使用、需要替换哪些内容，可选" disabled={busy} /></label>
      <details className="optional-images"><summary>参考图（可选）</summary><div className="edit-images">{draft.images.map((name, i) => <div className="edit-image" key={`${name}-${i}`}><img src={imageUrl(directory, name)} alt={`参考图 ${i + 1}`} /><div className="image-actions"><button disabled={busy || i === 0} onClick={() => setDraft(old => ({ ...old, images: [name, ...old.images.filter((_, j) => j !== i)] }))}>{i === 0 ? '封面' : '设为封面'}</button><button disabled={busy} aria-label={`移除第 ${i + 1} 张引用`} onClick={() => setDraft(old => ({ ...old, images: old.images.filter((_, j) => j !== i) }))}>移除</button></div></div>)}<button className="add-image" disabled={busy} onClick={() => imageFiles.current?.click()}><PlusIcon /><span>添加图片</span></button></div></details>
      <input ref={imageFiles} type="file" hidden multiple accept="image/png,image/jpeg,image/webp" onChange={e => { void addImages(e.target.files); }} />
      {error && <p className="inline-error" role="alert">{error}</p>}
    </div>
  </section>;
}
