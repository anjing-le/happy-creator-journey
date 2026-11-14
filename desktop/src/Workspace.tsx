import { useEffect, useRef, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { AssetLibrary, AssetEntry, AssetKind, emptyEntry, imageUrl, importImage, loadLibrary, saveEntry } from './library';

type Route = { page: 'home' } | { page: 'library' } | { page: 'creation'; name: string };
const HOME: Route = { page: 'home' };
const native = isTauri();
const entrances = [
  { name: '文', image: 'writing', description: '安静地写下想法，打磨成一篇文章' },
  { name: '图片', image: 'images', description: '从内容与提示词，做出封面和内页' },
  { name: '视频', image: 'video', description: '组织素材与 Skill，剪辑成一段视频' },
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
      <div className="creation-entries">{entrances.map(({ name, image, description }) => <button key={name} className={`entry-card entrance-${image}`} aria-label={name} aria-describedby={`entrance-${image}`} onClick={() => navigate({ page: 'creation', name })}>
        <span className="entrance-art"><img src={`/entrances/${image}-v1.png`} alt="" draggable={false} /></span>
        <span className="entrance-copy"><span className="entrance-name">{name}</span><span className="entrance-description" id={`entrance-${image}`}>{description}</span></span>
      </button>)}</div>
      <button className="library-entry" aria-label="素材库" aria-describedby="entrance-library" onClick={() => navigate({ page: 'library' })}>
        <img src="/entrances/library-v1.png" alt="" draggable={false} />
        <span className="entrance-copy"><span className="entrance-name">素材库</span><span className="entrance-description" id="entrance-library">角色、提示词、Skills 与参考素材，创作的十八般兵器</span></span>
      </button>
    </div>
  </section>;
  if (route.page === 'library') return <LibraryView back={back} />;
  return <section className="page"><header className="page-header"><button className="icon-button" onClick={back} aria-label="返回"><BackIcon /></button><h1>{route.name}</h1></header></section>;
}

function LibraryView({ back }: { back: () => void }) {
  const [library, setLibrary] = useState<AssetLibrary>({ entries: [], imageDirectory: '' });
  const [kind, setKind] = useState<AssetKind>('ip');
  const [selected, setSelected] = useState<AssetEntry>();
  const [editing, setEditing] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const refresh = async () => { setError(''); try { setLibrary(await loadLibrary()); } catch (error) { setError(message(error)); } finally { setLoading(false); } };
  useEffect(() => { void refresh(); }, []);
  const closeDetail = () => { setSelected(undefined); setEditing(false); };
  const create = () => { setSelected(emptyEntry(kind)); setEditing(true); };
  async function saved(entry: AssetEntry) { await refresh(); setSelected(entry); setEditing(false); }
  if (selected && editing) return <AssetEditor entry={selected} directory={library.imageDirectory} cancel={() => selected.id ? setEditing(false) : closeDetail()} saved={saved} />;
  if (selected) return <AssetDetail entry={selected} directory={library.imageDirectory} back={closeDetail} edit={() => setEditing(true)} />;
  const entries = library.entries.filter(entry => entry.kind === kind);
  return <section className="page" aria-label="素材库">
    <header className="page-header"><button className="icon-button" onClick={back} aria-label="返回首页"><BackIcon /></button><h1>素材库</h1><span className="header-space" /><button className="icon-button" onClick={create} aria-label={kind === 'ip' ? '添加角色 IP' : '添加创作场景'} title={kind === 'ip' ? '添加角色 IP' : '添加创作场景'}><PlusIcon /></button></header>
    <div className="library-content">
      <nav className="library-tabs" aria-label="素材类型"><button className={kind === 'ip' ? 'active' : ''} onClick={() => setKind('ip')}>角色 IP</button><button className={kind === 'scene' ? 'active' : ''} onClick={() => setKind('scene')}>创作场景</button></nav>
      {error && <div className="inline-error" role="alert">{error}<button onClick={() => { void refresh(); }}>重新读取</button></div>}
      {loading ? <div className="empty-library">正在打开…</div> : entries.length === 0 ? <div className="empty-library"><button className="empty-add" onClick={create}><PlusIcon /><span>{kind === 'ip' ? '添加一个角色' : '添加一套场景'}</span></button></div> : <div className="asset-grid">{entries.map(entry => <button className="asset-card" key={entry.id} onClick={() => setSelected(entry)}>
        <div className={`asset-art ${entry.kind === 'scene' && entry.images.length > 1 ? 'image-series' : ''}`}>{entry.images.length ? <>{entry.kind === 'scene' && entry.images.length > 1 && <div className="series-sheet" />}<img src={imageUrl(library.imageDirectory, entry.images[0])} alt={entry.name} loading="lazy" /></> : <div className="missing-art"><PlusIcon /></div>}</div>
        <span className="asset-name">{entry.name}</span>
      </button>)}</div>}
    </div>
  </section>;
}

function AssetDetail({ entry, directory, back, edit }: { entry: AssetEntry; directory: string; back: () => void; edit: () => void }) {
  const [index, setIndex] = useState(0);
  const [enlarged, setEnlarged] = useState(false);
  const [rulesOpen, setRulesOpen] = useState(false);
  const zoom = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    if (enlarged) zoom.current?.showModal(); else zoom.current?.close();
  }, [enlarged]);
  const instructions = entry.kind === 'ip' ? entry.prompt : entry.skillMarkdown;
  return <section className="page" aria-label={entry.name}>
    <header className="page-header"><button className="icon-button" onClick={back} aria-label="返回素材库"><BackIcon /></button><h1>{entry.name}</h1><span className="header-space" /><button className="text-button" onClick={edit}>编辑</button></header>
    <div className="asset-detail">
      <div className="detail-gallery">
        {entry.images.length > 0 ? <button className="main-art" aria-label="放大参考图" onClick={() => setEnlarged(true)}><img src={imageUrl(directory, entry.images[index])} alt={`${entry.name} ${index + 1}`} /></button> : <div className="main-art no-art">尚未添加样图</div>}
        {entry.images.length > 1 && <div className="filmstrip">{entry.images.map((name, i) => <button key={`${name}-${i}`} aria-label={`查看第 ${i + 1} 张`} aria-pressed={i === index} onClick={() => setIndex(i)}><img src={imageUrl(directory, name)} alt={`样图 ${i + 1}`} /></button>)}</div>}
      </div>
      <div className="rules-section"><button className="rules-toggle" aria-expanded={rulesOpen} onClick={() => setRulesOpen(!rulesOpen)}><span>{entry.kind === 'ip' ? '提示词' : 'Skill'}</span><span className="disclosure">{rulesOpen ? '−' : '+'}</span></button>{rulesOpen && <><pre className="rules-document">{instructions || (entry.kind === 'ip' ? '尚未添加提示词' : '尚未添加 Skill 文档')}</pre>{entry.source && <p className="asset-source">来源：{entry.source}</p>}</>}</div>
    </div>
    <dialog ref={zoom} className="image-lightbox" onCancel={event => { event.preventDefault(); setEnlarged(false); }} onClick={event => { if (event.target === event.currentTarget) setEnlarged(false); }}><button className="lightbox-close icon-button" onClick={() => setEnlarged(false)} aria-label="关闭大图">×</button>{entry.images[index] && <img src={imageUrl(directory, entry.images[index])} alt={entry.name} />}</dialog>
  </section>;
}

function AssetEditor({ entry, directory, cancel, saved }: { entry: AssetEntry; directory: string; cancel: () => void; saved: (entry: AssetEntry) => Promise<void> }) {
  const [draft, setDraft] = useState<AssetEntry>({ ...entry, images: [...entry.images] });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const files = useRef<HTMLInputElement>(null);
  const skillFile = useRef<HTMLInputElement>(null);
  const isIp = draft.kind === 'ip';
  async function addImages(selected: FileList | null) {
    if (!selected?.length || busy) return;
    if (draft.images.length + selected.length > 16) { setError('每份素材最多保存 16 张样图'); return; }
    setBusy(true); setError('');
    try { const names: string[] = []; for (const file of Array.from(selected)) names.push(await importImage(file)); setDraft(old => ({ ...old, images: [...old.images, ...names] })); }
    catch (error) { setError(message(error)); }
    finally { setBusy(false); if (files.current) files.current.value = ''; }
  }
  async function save() {
    if (busy) return;
    if (!native) { setError('请在 CreatorX 客户端中保存素材'); return; }
    setBusy(true); setError('');
    try { await saved(await saveEntry(draft)); } catch (error) { setError(message(error)); } finally { setBusy(false); }
  }
  return <section className="page" data-editing aria-label="编辑素材"><header className="page-header"><button className="icon-button" onClick={cancel} disabled={busy} aria-label="取消编辑"><BackIcon /></button><h1>{entry.id ? '编辑' : isIp ? '添加角色 IP' : '添加创作场景'}</h1><span className="header-space" /><button className="text-button save-button" onClick={() => { void save(); }} disabled={busy || !draft.name.trim()}>{busy ? '处理中…' : '保存'}</button></header>
    <div className="asset-editor">
      <label className="field-label">名称<input autoFocus maxLength={80} value={draft.name} onChange={event => setDraft({ ...draft, name: event.target.value })} placeholder={isIp ? '角色的名字' : '场景的名字'} disabled={busy} /></label>
      <div className="field-label">{isIp ? '参考图' : '样图套组'}<div className="edit-images">{draft.images.map((name, i) => <div className="edit-image" key={`${name}-${i}`}><img src={imageUrl(directory, name)} alt={`参考图 ${i + 1}`} /><div className="image-actions"><button disabled={busy || i === 0} aria-label={`将第 ${i + 1} 张设为封面`} onClick={() => setDraft(old => ({ ...old, images: [name, ...old.images.filter((_, j) => j !== i)] }))}>{i === 0 ? '封面' : '设为封面'}</button><button disabled={busy} aria-label={`移除第 ${i + 1} 张引用`} onClick={() => setDraft(old => ({ ...old, images: old.images.filter((_, j) => j !== i) }))}>移除</button></div></div>)}<button className="add-image" disabled={busy} onClick={() => files.current?.click()}><PlusIcon /><span>添加图片</span></button></div></div>
      <input ref={files} type="file" hidden multiple accept="image/png,image/jpeg,image/webp" onChange={event => { void addImages(event.target.files); }} />
      <div className="field-label rules-label"><label htmlFor="asset-rules">{isIp ? '提示词' : 'Skill 文档'}</label>{!isIp && <button className="text-button" disabled={busy} onClick={() => skillFile.current?.click()}>导入 SKILL.md</button>}</div>
      <textarea id="asset-rules" className="instructions-editor" value={isIp ? draft.prompt : draft.skillMarkdown} onChange={event => setDraft({ ...draft, [isIp ? 'prompt' : 'skillMarkdown']: event.target.value })} spellCheck={false} disabled={busy} placeholder={isIp ? '保持角色一致的提示词' : '这个场景的创作步骤、风格、规则与检查方法'} />
      <input ref={skillFile} type="file" hidden accept=".md,text/markdown,text/plain" onChange={async event => { const file = event.target.files?.[0]; if (!file) return; if (file.size > 256_000) { setError('Skill 文档过长'); return; } try { const text = await file.text(); setDraft(old => ({ ...old, skillMarkdown: text })); } catch { setError('无法读取 Skill 文档'); } }} />
      {error && <p className="inline-error" role="alert">{error}</p>}
    </div>
  </section>;
}
