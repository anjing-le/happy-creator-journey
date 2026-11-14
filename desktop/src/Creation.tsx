import { useEffect, useRef, useState } from 'react';
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import Markdown from 'react-markdown';
import { AssetEntry, loadLibrary } from './library';
interface Message { id: string; role: string; text: string }
interface Job { id: string; mode: string; title: string; skillName: string; skillId: string; promptIds: string[]; status: string; phase: string; messages: Message[]; images: string[]; error: string | null; turnId: string | null; activity: string }
const busy = (job?: Job) => !!job && ['starting', 'running'].includes(job.status);
const errorText = (e: unknown) => e instanceof Error ? e.message : String(e);
export function Creation({ name, back }: { name: string; back: () => void }) {
  const mode = name === '文' ? 'writing' : name === '视频' ? 'video' : 'image';
  const label = mode === 'writing' ? '文' : mode === 'video' ? '视频' : '图';
  const [assets, setAssets] = useState<AssetEntry[]>([]);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [selected, setSelected] = useState(() => localStorage.getItem(`creatorx.chat.${mode}`) || '');
  const [skillId, setSkillId] = useState('');
  const [promptIds, setPromptIds] = useState<string[]>([]);
  const [attachmentsChanged, setAttachmentsChanged] = useState(false);
  const [materialsOpen, setMaterialsOpen] = useState(false);
  const [text, setText] = useState('');
  const [connection, setConnection] = useState('正在连接…');
  const [directory, setDirectory] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [enlarged, setEnlarged] = useState<string>();
  const dialog = useRef<HTMLDialogElement>(null);
  const messagesView = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const composer = useRef<HTMLTextAreaElement>(null);
  const hydratedSelection = useRef('');
  const jobsRef = useRef(jobs); jobsRef.current = jobs;
  const submittingRef = useRef(submitting); submittingRef.current = submitting;
  const visibleJobs = jobs.filter(j => (j.mode || 'image') === mode);
  const job = visibleJobs.find(j => j.id === selected);
  const skills = assets.filter(a => a.kind === 'skill');
  const prompts = assets.filter(a => a.kind === 'prompt');
  async function refresh() {
    if (!isTauri()) return [];
    const result = await invoke<Job[]>('creation_list'); setJobs(result); jobsRef.current = result; return result;
  }
  useEffect(() => {
    if (!isTauri()) { setConnection('请在客户端中连接 Codex'); return; }
    let disposed = false;
    const materials = () => { void loadLibrary().then(lib => { if (!disposed) setAssets(lib.entries); }).catch(e => { if (!disposed) setError(errorText(e)); }); };
    materials(); void connect(); void refresh().catch(e => setError(errorText(e)));
    void invoke<string>('creation_directory').then(setDirectory).catch(e => setError(errorText(e)));
    const focus = () => { materials(); void refresh().catch(e => setError(errorText(e))); };
    window.addEventListener('focus', focus);
    const timer = setInterval(() => { if (!disposed && document.visibilityState === 'visible' && (submittingRef.current || jobsRef.current.some(busy))) void refresh().catch(e => setError(errorText(e))); }, 1000);
    return () => { disposed = true; clearInterval(timer); window.removeEventListener('focus', focus); };
  }, []);
  useEffect(() => { localStorage.setItem(`creatorx.chat.${mode}`, selected); }, [selected, mode]);
  useEffect(() => { if (job && hydratedSelection.current !== job.id) { hydratedSelection.current = job.id; setSkillId(job.skillId || ''); setPromptIds(job.promptIds || []); setAttachmentsChanged(false); } }, [job?.id]);
  useEffect(() => { if (enlarged) dialog.current?.showModal(); else dialog.current?.close(); }, [enlarged]);
  const lastMessage = job?.messages.at(-1)?.text;
  useEffect(() => { const view = messagesView.current; if (view && follow.current) view.scrollTop = view.scrollHeight; }, [lastMessage, job?.images.length, selected]);
  async function connect() {
    setConnection('正在连接…');
    try { await invoke('codex_status'); setConnection('Codex · 6.1 Sol · 中等'); }
    catch (e) { setConnection('连接未完成'); setError(errorText(e)); }
  }
  function select(j?: Job) {
    hydratedSelection.current = j?.id || ''; setSelected(j?.id || ''); setSkillId(j?.skillId || ''); setPromptIds(j?.promptIds || []);
    setAttachmentsChanged(false); setMaterialsOpen(false); setText(''); setError(''); follow.current = true; composer.current?.focus();
  }
  async function send() {
    const message = text.trim(); if (!message || submitting || busy(job)) return;
    setSubmitting(true); setError(''); follow.current = true;
    const previousIds = new Set(jobs.map(j => j.id));
    try {
      const result = job
        ? await invoke<Job>('creation_continue', { id: job.id, text: message, attachments: attachmentsChanged ? { skillId, promptIds } : null })
        : await invoke<Job>('creation_start', { mode, skillId: skillId || null, promptIds, brief: message });
      setSelected(result.id); setAttachmentsChanged(false); setMaterialsOpen(false);
      setText(current => current.trim() === message ? '' : current); await refresh();
    } catch (e) {
      setError(errorText(e)); const latest = await refresh().catch(() => []);
      if (!job) { const created = latest.find(j => !previousIds.has(j.id) && j.mode === mode); if (created) setSelected(created.id); }
    } finally { setSubmitting(false); composer.current?.focus(); }
  }
  const image = (relative: string) => convertFileSrc(`${directory}/${job?.id}/${relative}`);
  const locked = submitting || busy(job);
  async function openFile(path?: string) { if (!path || !job) return; try { await invoke('creation_open_file', { id: job.id, path }); } catch (e) { setError(errorText(e)); } }
  return <section className="page chat-page" aria-label={`${label}创作`}>
    <header className="page-header chat-header"><button className="icon-button" aria-label="返回首页" onClick={back}>‹</button><h1>{label}</h1><span className="header-space" /><button className="text-button codex-connection" onClick={() => { void connect(); }}>{connection}</button><button className="icon-button" aria-label="新建对话" onClick={() => select()} disabled={submitting}>＋</button></header>
    <div className={`chat-layout ${visibleJobs.length ? '' : 'no-history'}`}>
      {visibleJobs.length > 0 && <aside className="creation-history" aria-label={`${label}的对话`}>{[...visibleJobs].reverse().map(j => <button className={`creation-task ${selected === j.id ? 'selected' : ''}`} key={j.id} onClick={() => select(j)}><span>{j.title}</span>{busy(j) && <small>{j.activity}</small>}</button>)}</aside>}
      <div className="chat-main">
        <div className="chat-messages" ref={messagesView} onScroll={e => { const view = e.currentTarget; follow.current = view.scrollHeight - view.scrollTop - view.clientHeight < 100; }}>
          {!job && <div className="chat-empty"><p>{mode === 'writing' ? '想写点什么？' : mode === 'video' ? '想做一段什么视频？' : '想创作一张什么图？'}</p></div>}
          {job?.messages.map(m => <article key={m.id} className={`chat-message ${m.role}`} aria-label={m.role === 'user' ? '你的消息' : 'Codex 回复'}>{m.role === 'user' ? <p>{m.text}</p> : <Markdown skipHtml components={{ a: ({ href, children }) => href && /^https?:\/\//.test(href) ? <a href={href} target="_blank" rel="noreferrer">{children}</a> : <button className="inline-file-link" onClick={() => { void openFile(href); }}>{children}</button>, img: () => null }}>{m.text}</Markdown>}</article>)}
          {busy(job) && <p className="chat-activity" role="status">{job?.activity || 'Codex 正在回复…'}</p>}
          {job?.images.length ? <div className="chat-image-strip" aria-label="作品版本">{job.images.map((path, i) => <button key={path} aria-label={`放大作品 ${i + 1}`} onClick={() => setEnlarged(image(path))}><img src={image(path)} alt={`作品版本 ${i + 1}`} /><span>{i + 1}</span></button>)}</div> : null}
        </div>
        <div className="chat-composer-area">
          {(error || job?.error) && <p className="inline-error" role="alert">{error || job?.error}</p>}
          <div className="chat-composer" data-editing={text.trim() ? '' : undefined}>
            {materialsOpen && <div className="chat-materials"><label>Skill<select aria-label="选择 Skill（可选）" value={skillId} disabled={locked} onChange={e => { setSkillId(e.target.value); setAttachmentsChanged(true); }}><option value="">不指定，先和 Codex 交流</option>{skills.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label><details><summary>提示词与 IP{promptIds.length ? ` · ${promptIds.length}` : ''}</summary><div>{prompts.map(p => <label key={p.id}><input type="checkbox" checked={promptIds.includes(p.id)} disabled={locked || (!promptIds.includes(p.id) && promptIds.length >= 16)} onChange={e => { setPromptIds(old => e.target.checked ? [...old, p.id] : old.filter(id => id !== p.id)); setAttachmentsChanged(true); }} />{p.name}</label>)}</div></details></div>}
            <textarea ref={composer} autoFocus aria-label="与 Codex 对话" value={text} rows={3} onChange={e => setText(e.target.value)} onKeyDown={e => { if ((e.metaKey || e.ctrlKey) && e.key === 'Enter' && !e.nativeEvent.isComposing) { e.preventDefault(); void send(); } }} placeholder="和 Codex 说说你的想法…" />
            <div className="chat-composer-toolbar"><button className={`text-button ${materialsOpen ? 'active' : ''}`} aria-label="选择 Skill 和素材" aria-expanded={materialsOpen} onClick={() => setMaterialsOpen(!materialsOpen)}>＋</button><span className="chat-attached">{skillId ? skills.find(s => s.id === skillId)?.name : attachmentsChanged ? '' : job?.skillName || ''}{promptIds.length ? ` · ${promptIds.length} 条提示词` : ''}</span><span className="header-space" />{busy(job) ? <button className="chat-send stop" aria-label="停止生成" disabled={submitting || !job?.turnId} onClick={() => { setSubmitting(true); void invoke('creation_cancel', { id: job?.id }).then(refresh).catch(e => setError(errorText(e))).finally(() => setSubmitting(false)); }}>■</button> : <button className="chat-send" aria-label="发送给 Codex" disabled={locked || !text.trim()} onClick={() => { void send(); }}>{submitting ? '…' : '↑'}</button>}</div>
          </div>
        </div>
      </div>
    </div>
    <dialog ref={dialog} className="image-zoom" aria-label="查看作品原图" onClose={() => setEnlarged(undefined)} onClick={e => { if (e.target === e.currentTarget) setEnlarged(undefined); }}><button className="zoom-close" aria-label="关闭原图" onClick={() => setEnlarged(undefined)}>×</button>{enlarged && <img src={enlarged} alt="作品原图" />}</dialog>
  </section>;
}
