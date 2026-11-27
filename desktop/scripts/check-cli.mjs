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
  const first = call(['library', 'save', '--stdin'], JSON.stringify({
    kind: 'prompt',
    modality: 'image',
    name: '安静角色设定',
    prompt: '蛋黄黄乱发、黑卫衣、粗蜡笔',
    tags: ['#安静', '生图'],
  }));
  assert.equal(first.revision, 1);
  assert.deepEqual(first.tags, ['安静', '生图']);
  assert.equal(call(['library', 'get', first.id]).prompt, first.prompt);
  assert.equal(call(['library', 'list', '--modality', 'image', '--tag', '安静']).count, 1);
  assert.deepEqual(call(['library', 'tags']), ['安静', '生图']);
  const stale = spawnSync(binary, ['library', 'save', '--stdin', '--if-revision', '0'], { env, input: JSON.stringify(first), encoding: 'utf8', timeout: 15_000 });
  assert.equal(stale.status, 1); assert.equal(JSON.parse(stale.stderr).ok, false);
  call(['library', 'delete', first.id, '--if-revision', String(first.revision)]);
  assert.equal(call(['library', 'list']).count, 0);
  console.log('Packaged native CLI: JSON output, atomic modality + tag filtering, delete and stale-write protection passed.');
} finally { rmSync(root, { recursive: true, force: true }); }
