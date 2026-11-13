import { useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { downloadPercent, shouldCheckAtStartup } from './update-policy';

type Phase = 'idle' | 'checking' | 'available' | 'current' | 'installing' | 'restart' | 'error';
const native = isTauri();
const zh = navigator.language.toLowerCase().startsWith('zh');
const copy = zh ? {
  updates: '检查更新', close: '关闭', checking: '正在检查更新…', current: '已是最新版本',
  available: '发现新版本', install: '更新并重启', later: '稍后', retry: '重试',
  downloading: '正在下载并验证更新', finishing: '正在安装更新…', restarting: '正在重新启动…',
  failed: '暂时无法完成更新，请稍后重试。', preview: '请在 CreatorX 客户端中检查更新。',
  restart: '重新启动', restartFailed: '更新已安装，请重新启动 CreatorX。', version: '当前版本',
} : {
  updates: 'Check for updates', close: 'Close', checking: 'Checking for updates…', current: 'You’re up to date',
  available: 'Update available', install: 'Update and restart', later: 'Later', retry: 'Try again',
  downloading: 'Downloading and verifying update', finishing: 'Installing update…', restarting: 'Restarting…',
  failed: 'Unable to complete the update. Please try again later.', preview: 'Open CreatorX to check for updates.',
  restart: 'Restart', restartFailed: 'Update installed. Please restart CreatorX.', version: 'Current version',
};

function Arrow() {
  return <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 16V4m0 0L7.5 8.5M12 4l4.5 4.5M5 15v4a1 1 0 001 1h12a1 1 0 001-1v-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>;
}

export function App() {
  const [open, setOpen] = useState(false);
  const [phase, setPhase] = useState<Phase>('idle');
  const [version, setVersion] = useState('');
  const [nextVersion, setNextVersion] = useState('');
  const [notes, setNotes] = useState('');
  const [progress, setProgress] = useState<number>();
  const [message, setMessage] = useState('');
  const update = useRef<Update | null>(null);
  const checking = useRef(false);
  const installing = useRef(false);
  const modal = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const checkAction = useRef<(visible: boolean) => Promise<void>>(async () => {});

  async function checkUpdates(visible: boolean) {
    if (visible) setOpen(true);
    if (checking.current || installing.current) return;
    if (update.current) { setPhase('available'); return; }
    if (!native) { setPhase('error'); setMessage(copy.preview); return; }
    checking.current = true;
    setPhase('checking');
    try {
      const result = await check({ timeout: 20_000 });
      // Failed network requests must remain retryable on the next launch.
      localStorage.setItem('creatorx.last-update-check', String(Date.now()));
      update.current = result;
      if (result) {
        setNextVersion(result.version); setNotes(result.body ?? ''); setPhase('available');
      } else setPhase('current');
    } catch {
      setMessage(copy.failed); setPhase('error');
    } finally { checking.current = false; }
  }
  checkAction.current = checkUpdates;

  async function restart() {
    setMessage(copy.restarting);
    try { await relaunch(); }
    catch { setMessage(copy.restartFailed); setPhase('restart'); }
  }

  async function install() {
    const candidate = update.current;
    if (!candidate || installing.current) return;
    installing.current = true;
    setPhase('installing'); setProgress(undefined); setMessage(copy.downloading);
    let downloaded = 0;
    let total: number | undefined;
    try {
      await candidate.downloadAndInstall(event => {
        if (event.event === 'Started') total = event.data.contentLength;
        if (event.event === 'Progress') {
          downloaded += event.data.chunkLength; setProgress(downloadPercent(downloaded, total));
        }
        if (event.event === 'Finished') setMessage(copy.finishing);
      });
      update.current = null;
      setPhase('restart'); await restart();
    } catch {
      // Discard the failed resource; retry must obtain and verify a fresh update.
      update.current = null;
      await candidate.close().catch(() => {});
      setMessage(copy.failed); setPhase('error');
    } finally { installing.current = false; }
  }

  function dismiss() {
    if (installing.current || phase === 'restart') return;
    setOpen(false); trigger.current?.focus();
  }

  useEffect(() => {
    if (!native) return;
    void getVersion().then(setVersion);
    const stop = listen('creatorx:check-updates', () => { void checkAction.current(true); });
    const stopClose = getCurrentWindow().onCloseRequested(async event => {
      if (installing.current) { event.preventDefault(); return; }
      // Closing a macOS window keeps the app available from its Dock icon.
      if (navigator.userAgent.includes('Mac')) { event.preventDefault(); await getCurrentWindow().hide(); }
    });
    if (shouldCheckAtStartup(localStorage.getItem('creatorx.last-update-check'))) void checkAction.current(false);
    return () => {
      void stop.then(unlisten => unlisten()); void stopClose.then(unlisten => unlisten());
      void update.current?.close();
    };
  }, []);

  useEffect(() => {
    if (!open) return;
    modal.current?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); dismiss(); }
      if (event.key !== 'Tab') return;
      const items = modal.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)');
      if (!items?.length) { event.preventDefault(); return; }
      const first = items[0], last = items[items.length - 1];
      if (event.shiftKey && (document.activeElement === first || document.activeElement === modal.current)) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && (document.activeElement === last || document.activeElement === modal.current)) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [open, phase]);

  const busy = phase === 'installing';
  const title = phase === 'available' ? copy.available : phase === 'current' ? copy.current : copy.updates;
  return <main className="workspace" aria-label="CreatorX">
    <div className="drag-region" data-tauri-drag-region />
    <button ref={trigger} className={`update-trigger ${phase === 'available' ? 'has-update' : ''}`} aria-label={copy.updates} title={copy.updates} onClick={() => { void checkUpdates(true); }}><Arrow /></button>
    {open && <div className="scrim" onMouseDown={event => { if (event.target === event.currentTarget) dismiss(); }}>
      <div ref={modal} className="update-panel" role="dialog" aria-modal="true" aria-labelledby="update-title" tabIndex={-1}>
        {!busy && phase !== 'restart' && <button className="dismiss" onClick={dismiss} aria-label={copy.close}>×</button>}
        <div className="update-mark"><Arrow /></div>
        <h1 id="update-title">{title}</h1>
        <p className="version">CreatorX{version && ` · ${version}`}{phase === 'available' && ` → ${nextVersion}`}</p>
        <div className="status" role="status" aria-live="polite">
          {phase === 'checking' && <p>{copy.checking}</p>}
          {(phase === 'error' || busy || phase === 'restart') && <p>{message}</p>}
          {phase === 'available' && notes && <p className="release-notes">{notes}</p>}
          {busy && <progress value={progress} max="100" aria-label={copy.downloading} />}
        </div>
        <div className="actions">
          {phase === 'available' && <><button className="quiet" onClick={dismiss}>{copy.later}</button><button className="primary" onClick={() => { void install(); }}>{copy.install}</button></>}
          {phase === 'error' && native && <button className="primary" onClick={() => { void checkUpdates(true); }}>{copy.retry}</button>}
          {phase === 'current' && <button className="quiet" onClick={dismiss}>{copy.close}</button>}
          {phase === 'restart' && <button className="primary" onClick={() => { void restart(); }}>{copy.restart}</button>}
        </div>
      </div>
    </div>}
  </main>;
}
