const { test } = require('node:test');
const assert = require('node:assert/strict');
const { quotaSummary } = require('../src/quota-state.js');
const snapshot = (pct, extra = {}) => ({ fetched_at: 1000, windows: [{used_percent:pct}], ...extra });
test('all failures and empty states remain unknown, not green zero', () => {
  assert.equal(quotaSummary([], 1000).worst, null);
  assert.deepEqual(quotaSummary([snapshot(0, {error:'offline',windows:[]})], 1000), {worst:null, unavailable:1});
});
test('stale values and errors do not mask healthy or dangerous current values', () => {
  assert.deepEqual(quotaSummary([snapshot(99,{error:'offline'}), snapshot(20)], 1000), {worst:20,unavailable:1});
  assert.equal(quotaSummary([snapshot(95)], 1601).worst, null);
  assert.equal(quotaSummary([snapshot(95,{stale_after_secs:3660})], 2000).worst, 95);
  assert.equal(quotaSummary([snapshot(0)], 1000).worst, 0);
  assert.equal(quotaSummary([snapshot(95), snapshot(20)], 1000).worst, 95);
});
