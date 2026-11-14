import { useEffect, useRef, useState } from 'react';
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import { AssetEntry, loadLibrary } from './library';
interface Message { id: string; role: string; text: string }
interface Job { id: string; title: string; skillName: string; threadId: string | null; status: string; phase: string; proposal: string; confirmed: string; messages: Message[]; images: string[]; error: string | null; turnId: string | null; activity: string }
const busy = (job?: Job) => !!job && ['starting', 'running'].includes(job.status);
const errorText = (e: unknown) => e instanceof Error ? e.message : String(e);
export function Creation({ name, back }: { name: string; back: () => void }) {
  const [assets, setAssets] = useState<AssetEntry[]>([]);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [selected, setSelected] = useState<string>(() => localStorage.getItem('creatorx.creation.selected') || '');
  const [skillId, setSkillId] = useState('');
  const [promptIds, setPromptIds] = useState<string[]>([]);
  const [brief, setBrief] = useState('');
  const [draft, setDraft] = useState('');
  const [feedback, setFeedback] = useState('');
  const [connection, setConnection] = useState('尚未连接');
  const [directory, setDirectory] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [enlarged, setEnlarged] = useState<string>();
  const dialog = useRef<HTMLDialogElement>(null);
  const initializedDraft = useRef('');
  const job = jobs.find(j => j.id === selected);
  const skills = assets.filter(a => a.kind === 'skill');
  const prompts = assets.filter(a => a.kind === 'prompt');
  async function refresh() { if (isTauri()) setJobs(await invoke<Job[]>('creation_list')); }
  useEffect(() => {
    if (!isTauri()) { setError('请在 CreatorX 客户端中连接本机 Codex'); return; }
    let disposed = false;
    void loadLibrary().then(lib => { if (!disposed) { setAssets(lib.entries); setSkillId(lib.entries.find(e => e.kind === 'skill')?.id || ''); } }).catch(e => setError(errorText(e)));
    void invoke<string>('creation_directory').then(setDirectory).catch(e => setError(errorText(e)));
    void connect(); void refresh().catch(e => setError(errorText(e)));
    const timer = setInterval(() => { if (!disposed) void refresh().catch(e => setError(errorText(e))); }, 1200);
    return () => { disposed = true; clearInterval(timer); };
  }, []);
  useEffect(() => { localStorage.setItem('creatorx.creation.selected', selected); }, [selected]);
  useEffect(() => {
    if (job?.proposal && initializedDraft.current !== `${job.id}:${job.proposal}`) { setDraft(job.proposal); initializedDraft.current = `${job.id}:${job.proposal}`; }
  }, [job?.id, job?.proposal]);
  useEffect(() => { if (enlarged) dialog.current?.showModal(); else dialog.current?.close(); }, [enlarged]);
  async function connect() {
    setConnection('正在连接…');
    try { await invoke('codex_status'); setConnection('Codex · 6.1 Sol · 中等'); }
    catch (e) { setConnection('连接未完成'); setError(errorText(e)); }
  }
  async function submit(phase?: 'draft' | 'generate' | 'revise') {
    if (submitting || busy(job)) return;
    setSubmitting(true); setError('');
    try {
      const result = phase && job
        ? await invoke<Job>('creation_continue', { id: job.id, phase, text: phase === 'generate' ? draft : feedback })
        : await invoke<Job>('creation_start', { skillId, promptIds, brief });
      setSelected(result.id); await refresh(); setFeedback('');
    } catch (e) { setError(errorText(e)); await refresh().catch(() => {}); }
    finally { setSubmitting(false); }
  }
  const image = (relative: string) => convertFileSrc(`${directory}/${job?.id}/${relative}`);
  const locked = submitting || busy(job);
  const draftComplete = !!job?.proposal && !busy(job) && job.phase === 'draft';
  return <section className="page creation-page" aria-label="图文创作">
    <header className="page-header"><button className="icon-button" aria-label="返回首页" onClick={back}>‹</button><h1>{name === '视频' ? '视频' : '图文创作'}</h1><span className="header-space" /><button className="text-button codex-connection" onClick={() => { void connect(); }}>{connection}</button><button className="text-button" disabled={submitting} onClick={() => { setSelected(''); setError(''); }}>新建创作</button></header>
    <div className="creation-layout">
      <aside className="creation-history" aria-label="创作任务">{jobs.length ? [...jobs].reverse().map(j => <button className={`creation-task ${selected === j.id ? 'selected' : ''}`} key={j.id} onClick={() => { setSelected(j.id); setError(''); setFeedback(''); }}><span>{j.title}</span><small>{busy(j) ? j.activity : j.images.length ? `${j.images.length} 张作品` : j.status === 'completed' ? '待确认内容' : '待继续'}</small></button>) : <p className="sidebar-note">作品与修改过程会留在这里。</p>}</aside>
      <div className="creation-main">{error && <p className="inline-error" role="alert">{error}</p>}
        {!job ? <div className="creation-setup" data-editing>
          <p className="step-caption">先选方法，再说想做什么</p>
          <label className="field-label">创作 Skill<select aria-label="创作 Skill" value={skillId} onChange={e => setSkillId(e.target.value)} disabled={submitting}><option value="">选择素材库中的 Skill</option>{skills.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
          {!skills.length && <p className="sidebar-note">先在素材库导入一个完整 Skill 文件夹。</p>}
          <details className="creation-materials"><summary>IP、风格与附加提示词{promptIds.length ? ` · 已选 ${promptIds.length}` : '（可选）'}</summary><div>{prompts.map(p => <label key={p.id}><input type="checkbox" checked={promptIds.includes(p.id)} disabled={submitting || (!promptIds.includes(p.id) && promptIds.length >= 16)} onChange={e => setPromptIds(old => e.target.checked ? [...old, p.id] : old.filter(id => id !== p.id))} />{p.name}</label>)}</div></details>
          <label className="field-label">这次想创作什么<textarea aria-label="创作内容" rows={7} value={brief} onChange={e => setBrief(e.target.value)} placeholder="给谁看、想讲清什么、已有内容……" disabled={submitting} /></label>
          <div className="creation-actions"><span>先整理文稿，不会直接出图</span><button className="primary" disabled={submitting || !skillId || !brief.trim()} onClick={() => { void submit(); }}>{submitting ? '正在提交…' : '整理待确认内容'}</button></div>
        </div> : <div className="creation-work">
          <div className="creation-task-heading"><div><p className="step-caption">{job.skillName}</p><h2>{job.title}</h2></div>{busy(job) && <button className="text-button" disabled={submitting || !job.turnId} onClick={() => { setSubmitting(true); void invoke('creation_cancel', { id: job.id }).then(refresh).catch(e => setError(errorText(e))).finally(() => setSubmitting(false)); }}>停止</button>}</div>
          <p className="creation-activity" role="status">{job.activity || '准备继续'}</p>
          {job.error && <p className="inline-error" role="alert">{job.error}</p>}
          {draftComplete && <div className="confirmation-panel" data-editing><label className="field-label">确认内容<textarea aria-label="确认内容" value={draft} onChange={e => setDraft(e.target.value)} rows={14} /></label><div className="creation-actions"><span>可直接修改，确认后生成图片</span><button className="primary" disabled={locked || !draft.trim()} onClick={() => { void submit('generate'); }}>确认内容，生成图片</button></div></div>}
          {job.images.length > 0 && <div className="creation-gallery" aria-label="作品版本">{job.images.map((path, i) => <figure key={path}><button onClick={() => setEnlarged(image(path))} aria-label={`放大作品 ${i + 1}`}><img src={image(path)} alt={`创作图片 ${i + 1}`} /></button><figcaption>第 {i + 1} 张 · 保留原图</figcaption></figure>)}</div>}
          <details className="creation-conversation" open={busy(job)}><summary>创作过程</summary>{job.messages.map(m => <div key={m.id} className={`creation-message ${m.role}`}><small>{m.role === 'user' ? '你' : 'Codex'}</small><pre>{m.text}</pre></div>)}</details>
          {!busy(job) && <div className="creation-feedback" data-editing><textarea aria-label="修改意见" rows={3} value={feedback} disabled={locked} onChange={e => setFeedback(e.target.value)} placeholder={job.confirmed ? '告诉 Codex 哪里需要调整，沿用当前内容与图片继续修改…' : '补充要求，继续打磨待确认内容…'} /><div className="creation-actions"><span>沿用当前任务、Skill 与素材</span><button className="primary" disabled={locked || !feedback.trim()} onClick={() => { void submit(job.confirmed ? 'revise' : 'draft'); }}>{submitting ? '正在提交…' : '继续修改'}</button></div></div>}
        </div>}
      </div>
    </div>
    <dialog ref={dialog} className="image-zoom" aria-label="查看作品原图" onClose={() => setEnlarged(undefined)} onClick={e => { if (e.target === e.currentTarget) setEnlarged(undefined); }}><button className="zoom-close" aria-label="关闭原图" onClick={() => setEnlarged(undefined)}>×</button>{enlarged && <img src={enlarged} alt="作品原图" />}</dialog>
  </section>;
}
