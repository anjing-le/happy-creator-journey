import test from 'node:test';
import assert from 'node:assert/strict';
import { CHECK_INTERVAL, downloadPercent, shouldCheckAtStartup } from '../src/update-policy.ts';
test('startup checks are throttled while bad or future timestamps recover', () => {
  const now = 10 * CHECK_INTERVAL;
  assert.equal(shouldCheckAtStartup(null, now), true);
  assert.equal(shouldCheckAtStartup(String(now - CHECK_INTERVAL + 1), now), false);
  assert.equal(shouldCheckAtStartup(String(now - CHECK_INTERVAL), now), true);
  for (const value of ['bad', '', '0', '-1', String(now + 1)]) assert.equal(shouldCheckAtStartup(value, now), true);
});
test('unknown-length downloads remain indeterminate and progress stays bounded', () => {
  assert.equal(downloadPercent(100), undefined);
  assert.equal(downloadPercent(100, 0), undefined);
  assert.equal(downloadPercent(50, 100), 50);
  assert.equal(downloadPercent(110, 100), 100);
  assert.equal(downloadPercent(-10, 100), 0);
});
