import { useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { StudioEntrance, StudioLaunch } from './navigation';
import {
  AssetEntry,
  AssetKind,
  AssetLibrary,
  MODALITY_LABELS,
  Modality,
  imageUrl,
  loadLibrary,
  loadLibraryToken,
  openSkillFolder,
  skillFiles,
} from './library';

const native = isTauri();

function BackIcon() {
  return <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m14 6-6 6 6 6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>;
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
  const [library, setLibrary] = useState<AssetLibrary>({ entries: [], imageDirectory: '', tags: [], stateToken: '' });
  const [builtins, setBuiltins] = useState<AssetEntry[]>([]);
  const [kindFilter, setKindFilter] = useState<'all' | AssetKind>('all');
  const [modalityFilter, setModalityFilter] = useState<'all' | Modality>('all');
  const [tagFilter, setTagFilter] = useState<string>('');
  const [selected, setSelected] = useState<AssetEntry>();
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const tokenRef = useRef('');

  async function refresh() {
    try {
      const [lib, built] = await Promise.all([
        loadLibrary(),
        native ? invoke<AssetEntry[]>('creation_builtin_skills') : Promise.resolve([]),
      ]);
      tokenRef.current = lib.stateToken;
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
    if (!native) return;
    let disposed = false;
    const checkSync = async () => {
      if (disposed || document.visibilityState !== 'visible') return;
      try {
        const nextToken = await loadLibraryToken();
        if (nextToken && nextToken !== tokenRef.current) await refresh();
      } catch {
        // Ignore transient background check errors.
      }
    };
    const onFocus = () => { void checkSync(); };
    window.addEventListener('focus', onFocus);
    const timer = setInterval(() => { void checkSync(); }, 1500);
    return () => {
      disposed = true;
      clearInterval(timer);
      window.removeEventListener('focus', onFocus);
    };
  }, []);

  const allEntries = [...builtins, ...library.entries];
  const allTags = [...new Set([...library.tags, ...builtins.flatMap(b => b.tags)])];

  const filtered = allEntries.filter(entry => {
    if (kindFilter !== 'all' && entry.kind !== kindFilter) return false;
    if (modalityFilter !== 'all' && entry.modality !== modalityFilter) return false;
    if (tagFilter && !entry.tags.includes(tagFilter)) return false;
    return true;
  });

  return (
    <section className="page showcase-page" aria-label="样板间">
      <header className="page-header showcase-header">
        <button className="icon-button" onClick={back} aria-label="返回首页"><BackIcon /></button>
        <nav className="library-tabs compact-tabs" aria-label="基础类型">
          <button className={kindFilter === 'all' ? 'active' : ''} onClick={() => { setKindFilter('all'); setSelected(undefined); }}>
            全部<span className="tab-count">{allEntries.length}</span>
          </button>
          <button className={kindFilter === 'prompt' ? 'active' : ''} onClick={() => { setKindFilter('prompt'); setSelected(undefined); }}>
            提示词<span className="tab-count">{library.entries.filter(e => e.kind === 'prompt').length}</span>
          </button>
          <button className={kindFilter === 'skill' ? 'active' : ''} onClick={() => { setKindFilter('skill'); setSelected(undefined); }}>
            Skill<span className="tab-count">{allEntries.filter(e => e.kind === 'skill').length}</span>
          </button>
        </nav>
        <span className="header-space" />
        <div className="segmented-pills" role="group" aria-label="方向">
          {(['all', 'text', 'image', 'video'] as const).map(m => (
            <button key={m} className={modalityFilter === m ? 'active' : ''} onClick={() => { setModalityFilter(m); setSelected(undefined); }}>
              {m === 'all' ? '全部' : MODALITY_LABELS[m]}
            </button>
          ))}
        </div>
      </header>

      {error && <div className="browser-error inline-error" role="alert">{error}</div>}

      {selected ? (
        <ShowcaseReader
          entry={selected}
          directory={library.imageDirectory}
          close={() => setSelected(undefined)}
          pickTag={tag => { setTagFilter(tag); setSelected(undefined); }}
          useInStudio={() => {
            const target = launchTarget(selected);
            onLaunch(target.entrance, target.launch);
          }}
        />
      ) : (
        <div className="showcase-body">
          {allTags.length > 0 && (
            <div className="showcase-subfilters" aria-label="标签">
              <button className={`filter-chip ${!tagFilter ? 'active' : ''}`} onClick={() => setTagFilter('')}>全部</button>
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
            <div className="empty-library"><p>正在载入样板间…</p></div>
          ) : filtered.length === 0 ? (
            <div className="empty-library">
              <p>当前筛选下暂无样板 · 通过 Agent（MCP / CLI）即可维护提示词、Skill 与 Tag</p>
            </div>
          ) : (
            <div className="showcase-grid">
              {filtered.map(entry => {
                const isBuiltin = entry.id.startsWith('builtin:');
                const previewText = entry.sample.trim() || entry.notes.trim() || (entry.kind === 'prompt' ? entry.prompt : entry.skillMarkdown);
                return (
                  <article key={entry.id} className="showcase-card">
                    <button className="showcase-card-cover" onClick={() => setSelected(entry)} aria-label={`查看 ${entry.name}`}>
                      {entry.images.length > 0 ? (
                        <div className="showcase-cover-art">
                          {entry.images.length > 1 && <span className="series-sheet" aria-hidden="true" />}
                          <img src={imageUrl(library.imageDirectory, entry.images[0])} alt={entry.name} loading="lazy" />
                          {entry.images.length > 1 && <span className="image-count-badge">{entry.images.length} 张</span>}
                        </div>
                      ) : (
                        <div className="showcase-cover-text">
                          <span className="sample-kicker">{entry.sample.trim() ? '样板效果' : entry.kind === 'skill' ? 'Skill' : '提示词'}</span>
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

function ShowcaseReader({
  entry, directory, close, pickTag, useInStudio,
}: {
  entry: AssetEntry;
  directory: string;
  close: () => void;
  pickTag: (tag: string) => void;
  useInStudio: () => void;
}) {
  const [index, setIndex] = useState(0);
  const [enlarged, setEnlarged] = useState(false);
  const [files, setFiles] = useState<string[]>([]);
  const [error, setError] = useState('');
  const zoom = useRef<HTMLDialogElement>(null);
  const isBuiltin = entry.id.startsWith('builtin:');
  const isPrompt = entry.kind === 'prompt';

  useEffect(() => { if (enlarged) zoom.current?.showModal(); else zoom.current?.close(); }, [enlarged]);
  useEffect(() => {
    if (!isPrompt && !isBuiltin) void skillFiles(entry.id).then(setFiles).catch(e => setError(errText(e)));
    else if (!isPrompt && isBuiltin) setFiles(['SKILL.md']);
  }, [entry.id, isPrompt, isBuiltin]);

  return (
    <div className="showcase-body">
      <article className="showcase-detail-panel" aria-label={entry.name}>
        <header className="material-heading">
          <button className="icon-button" onClick={close} aria-label="返回样板间"><BackIcon /></button>
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
          {!isPrompt && !isBuiltin && (
            <button className="text-button" onClick={() => { void openSkillFolder(entry.id).catch(e => setError(errText(e))); }}>
              打开本地目录
            </button>
          )}
          <button className="text-button primary-action" onClick={useInStudio}>在创作中引用 →</button>
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
            <span className="sample-kicker">样板效果</span>
            <p>{entry.sample}</p>
          </div>
        )}

        {entry.notes && <p className="material-notes">{entry.notes}</p>}

        {!isPrompt && (
          <details className="skill-package" open>
            <summary>Skill 文件列表 · {files.length}</summary>
            <ul className="skill-file-tree">{files.map(path => <li key={path}>{path}</li>)}</ul>
          </details>
        )}

        <details className="rules-section" open={entry.images.length === 0 && !entry.sample.trim()}>
          <summary className="rules-toggle">{isPrompt ? '提示词内容' : 'SKILL.md 内容'}</summary>
          <pre className="rules-document">{isPrompt ? entry.prompt : entry.skillMarkdown}</pre>
        </details>

        {error && <p className="inline-error" role="alert">{error}</p>}

        <dialog ref={zoom} className="image-lightbox" onCancel={e => { e.preventDefault(); setEnlarged(false); }} onClick={e => { if (e.target === e.currentTarget) setEnlarged(false); }}>
          <button className="lightbox-close icon-button" onClick={() => setEnlarged(false)} aria-label="关闭大图">×</button>
          {entry.images[index] && <img src={imageUrl(directory, entry.images[index])} alt={entry.name} />}
        </dialog>
      </article>
    </div>
  );
}
