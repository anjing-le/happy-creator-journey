import assert from 'node:assert/strict';
import test from 'node:test';
import { initialNavigation, navigationKey, navigationMouse, navigationReducer } from '../src/navigation.ts';

test('back and forward restore visited routes and clamp at both ends', () => {
  let state = navigationReducer(initialNavigation, { type: 'visit', route: { page: 'studio', name: '图文' } });
  state = navigationReducer(state, { type: 'back' });
  assert.equal(state.history[state.position].page, 'home');
  assert.equal(navigationReducer(state, { type: 'back' }).position, 0);
  state = navigationReducer(state, { type: 'forward' });
  assert.deepEqual(state.history[state.position], { page: 'studio', name: '图文' });
  assert.equal(navigationReducer(state, { type: 'forward' }).position, 1);
});

test('visiting showcase after going back replaces the forward branch', () => {
  let state = navigationReducer(initialNavigation, { type: 'visit', route: { page: 'studio', name: '图文' } });
  state = navigationReducer(state, { type: 'back' });
  state = navigationReducer(state, { type: 'visit', route: { page: 'showcase' } });
  assert.deepEqual(state.history, [{ page: 'home' }, { page: 'showcase' }]);
  assert.equal(navigationReducer(state, { type: 'forward' }).position, 1);
  assert.equal(navigationReducer(state, { type: 'visit', route: { page: 'showcase' } }), state);
});

test('launching studio from showcase records exact template launch route', () => {
  let state = initialNavigation;
  state = navigationReducer(state, { type: 'visit', route: { page: 'showcase' } });
  state = navigationReducer(state, { type: 'visit', route: { page: 'studio', name: '图文', launch: { deliverable: 'article', skillId: 'builtin:article-illustrated', track: '计算机', nonce: 1 } } });
  state = navigationReducer(state, { type: 'visit', route: { page: 'studio', name: '视频' } });
  state = navigationReducer(navigationReducer(state, { type: 'back' }), { type: 'back' });
  assert.deepEqual(state.history[state.position], { page: 'showcase' });
});

test('Command brackets work with physical bracket keys and ignore editing modifiers or IME composition', () => {
  const key = { key: '[', code: 'BracketLeft', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, isComposing: false };
  assert.equal(navigationKey(key), 'back');
  assert.equal(navigationKey({ ...key, key: '【' }), 'back');
  assert.equal(navigationKey({ ...key, key: ']', code: 'BracketRight' }), 'forward');
  assert.equal(navigationKey({ ...key, isComposing: true }), null);
  assert.equal(navigationKey({ ...key, metaKey: false }), null);
  assert.equal(navigationKey({ ...key, shiftKey: true }), null);
  assert.equal(navigationKey({ ...key, metaKey: false, altKey: true, key: 'ArrowLeft', code: 'ArrowLeft' }), 'back');
  assert.equal(navigationKey({ ...key, metaKey: false, altKey: true, key: 'ArrowRight', code: 'ArrowRight' }), 'forward');
  assert.equal(navigationMouse(3), 'back');
  assert.equal(navigationMouse(4), 'forward');
  for (const button of [0, 1, 2]) assert.equal(navigationMouse(button), null);
});
