import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
const binary = realpathSync(process.argv[2]);
const root = mkdtempSync(path.join(tmpdir(), 'creatorx-cli-check-'));
const env = { ...process.env, CREATORX_LIBRARY_DIR: root };
function call(args, input) {
  const response = JSON.parse(execFileSync(binary, args, { env, input, encoding: 'utf8', timeout: 15_000 }));
  assert.equal(response.ok, true); return response.data;
}
try {
  assert.equal(call(['schema']).name, 'CreatorX CLI');
  const first = call(['library', 'save', '--stdin'], JSON.stringify({ kind: 'prompt', name: 'CLI check', prompt: 'A temporary CLI test.' }));
  assert.equal(first.revision, 1);
  assert.equal(call(['library', 'get', first.id]).prompt, first.prompt);
  assert.equal(call(['library', 'list']).count, 1);
  const stale = spawnSync(binary, ['library', 'save', '--stdin', '--if-revision', '0'], { env, input: JSON.stringify(first), encoding: 'utf8', timeout: 15_000 });
  assert.equal(stale.status, 1); assert.equal(JSON.parse(stale.stderr).ok, false);
  call(['library', 'add-category', '--name', 'Empty category']);
  assert.ok(call(['library', 'categories']).includes('Empty category'));
  const savedIp = call(['library', 'save-ip', '--stdin'], JSON.stringify({ name: 'Anjing', visualPrompt: 'crayon', images: [], voicePrompt: 'concise', voiceSample: '', tracks: [{ name: '计算机', positioning: 'AI' }], revision: 0 }));
  assert.equal(savedIp.revision, 1);
  assert.equal(call(['library', 'ip']).name, 'Anjing');
  call(['library', 'delete', first.id, '--if-revision', String(first.revision)]);
  assert.equal(call(['library', 'list']).count, 0);
  console.log('Packaged native CLI: JSON output, IP foundation, shared storage, empty categories, delete and stale-write protection passed.');
} finally { rmSync(root, { recursive: true, force: true }); }
