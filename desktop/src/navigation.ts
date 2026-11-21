export type Route = { page: 'home' } | { page: 'library' } | { page: 'creation'; name: string };
export interface Navigation { history: Route[]; position: number }
export type NavigationAction = { type: 'visit'; route: Route } | { type: 'back' | 'forward' };
export const initialNavigation: Navigation = { history: [{ page: 'home' }], position: 0 };
export function navigationReducer(state: Navigation, action: NavigationAction): Navigation {
  if (action.type === 'visit') {
    if (JSON.stringify(state.history[state.position]) === JSON.stringify(action.route)) return state;
    return { history: [...state.history.slice(0, state.position + 1), action.route], position: state.position + 1 };
  }
  return { ...state, position: Math.max(0, Math.min(state.history.length - 1, state.position + (action.type === 'back' ? -1 : 1))) };
}
interface Key { key: string; code: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; isComposing: boolean }
export function navigationKey(event: Key): 'back' | 'forward' | null {
  if (event.isComposing || event.shiftKey || event.ctrlKey) return null;
  if (event.metaKey && !event.altKey) {
    if (event.code === 'BracketLeft' || event.key === '[') return 'back';
    if (event.code === 'BracketRight' || event.key === ']') return 'forward';
  }
  if (event.altKey && !event.metaKey) {
    if (event.key === 'ArrowLeft') return 'back';
    if (event.key === 'ArrowRight') return 'forward';
  }
  return null;
}
export function navigationMouse(button: number): 'back' | 'forward' | null {
  return button === 3 ? 'back' : button === 4 ? 'forward' : null;
}
