const assert = require('node:assert/strict');
const test = require('node:test');
const { selectCandidates } = require('./rustsec-scan.cjs');

const fresh = 'RUSTSEC-2099-0001';
const tracked = 'RUSTSEC-2099-0002';
const advisory = (id, severity = 'error') => ({
  type: 'diagnostic',
  fields: { severity, advisory: { id } },
});
const summary = (errors) => ({ type: 'summary', fields: { advisories: { errors } } });

test('a completed clean scan has no candidates', () => {
  assert.deepEqual(selectCandidates([summary(0)], 0, []), []);
});

test('an open issue or PR title containing the exact ID suppresses that finding', () => {
  const records = [advisory(tracked), summary(1)];
  assert.deepEqual(selectCandidates(records, 1, [`[RustSec] ${tracked}: upgrade`]), []);
});

test('untracked IDs are unique and similar title text does not suppress them', () => {
  const records = [advisory(tracked), advisory(fresh), advisory(fresh, 'warning'), summary(2)];
  assert.deepEqual(selectCandidates(records, 1, [tracked, `${fresh}0`]), [fresh]);
});

test('incomplete scans, tool errors, and invalid findings fail closed', () => {
  const invalidScans = [
    [[advisory(fresh)], 1],
    [[advisory(fresh), summary(0)], 1],
    [[summary(0)], 1],
    [[{ type: 'log', fields: { level: 'ERROR' } }, summary(0)], 0],
    [[{ type: 'diagnostic', fields: { severity: 'error' } }, summary(1)], 1],
    [[advisory('not an advisory ID'), summary(1)], 1],
  ];

  for (const [records, exitCode] of invalidScans) {
    assert.throws(() => selectCandidates(records, exitCode, []));
  }
});
