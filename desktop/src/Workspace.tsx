import { useEffect, useReducer } from 'react';
import { Creation } from './Creation';
import { Showcase } from './Showcase';
import { initialNavigation, navigationKey, navigationMouse, navigationReducer, Route, StudioEntrance } from './navigation';

const HOME: Route = { page: 'home' };
const entrances: { name: StudioEntrance; image: string }[] = [
  { name: '图文', image: 'images' },
  { name: '视频', image: 'video' },
];

export function Workspace() {
  const [{ history, position }, dispatch] = useReducer(navigationReducer, initialNavigation);
  const route = history[position];
  const navigate = (next: Route) => dispatch({ type: 'visit', route: next });
  const back = () => dispatch({ type: 'back' });

  useEffect(() => {
    const blocked = () => !!document.querySelector('dialog[open], [role="dialog"], [data-editing], [role="combobox"][aria-expanded="true"]');
    const key = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing) return;
      const direction = navigationKey(event);
      if (direction) {
        event.preventDefault();
        if (!blocked()) dispatch({ type: direction });
        return;
      }
      if (blocked() || event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || (event.target as HTMLElement)?.isContentEditable) return;
      if ((event.metaKey || event.ctrlKey) && event.key === '1') {
        event.preventDefault();
        dispatch({ type: 'visit', route: HOME });
      }
    };
    const suppressNativeHistory = (event: MouseEvent) => {
      if (navigationMouse(event.button)) event.preventDefault();
    };
    const mouse = (event: MouseEvent) => {
      const direction = navigationMouse(event.button);
      if (!direction) return;
      event.preventDefault();
      if (!blocked()) dispatch({ type: direction });
    };
    window.addEventListener('keydown', key);
    window.addEventListener('mousedown', suppressNativeHistory);
    window.addEventListener('mouseup', mouse);
    window.addEventListener('auxclick', suppressNativeHistory);
    return () => {
      window.removeEventListener('keydown', key);
      window.removeEventListener('mousedown', suppressNativeHistory);
      window.removeEventListener('mouseup', mouse);
      window.removeEventListener('auxclick', suppressNativeHistory);
    };
  }, []);

  if (route.page === 'home') {
    return (
      <section className="home" aria-label="CreatorX 首页">
        <div className="home-entries">
          <div className="creation-entries">
            {entrances.map(({ name, image }) => (
              <button
                key={name}
                className={`entry-card entrance-${image}`}
                aria-label={name}
                onClick={() => navigate({ page: 'studio', name })}
              >
                <span className="entrance-art">
                  <img src={`/entrances/${image}-v2.png`} alt="" draggable={false} />
                </span>
              </button>
            ))}
          </div>
          <button
            className="library-entry home-library-region"
            aria-label="样板间"
            onClick={() => navigate({ page: 'showcase' })}
          >
            <img src="/entrances/library-v2.png" alt="" draggable={false} />
          </button>
        </div>
      </section>
    );
  }

  if (route.page === 'showcase') {
    return (
      <Showcase
        back={back}
        onLaunch={(name, launch) => navigate({ page: 'studio', name, launch })}
      />
    );
  }

  return <Creation key={route.name} name={route.name} launch={route.launch} back={back} />;
}
