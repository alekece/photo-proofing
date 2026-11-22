const assert = require('node:assert/strict');

function readFindings(records, exitCode) {
  const findings = new Set();
  let summary;
  let errors = 0;

  for (const { type, fields } of records) {
    assert(!(type === 'log' && fields?.level === 'ERROR'), 'cargo-deny reported a tool error');

    if (type === 'summary') {
      assert(summary === undefined, 'cargo-deny emitted multiple summaries');
      summary = fields?.advisories ?? null;
    }

    if (type !== 'diagnostic') continue;

    assert(fields?.code !== 'index-failure', 'cargo-deny could not check the registry index');

    if (fields?.severity === 'error') {
      assert(fields.advisory, 'cargo-deny reported a non-advisory error');
      errors++;
    }

    if (fields?.advisory) {
      const { id } = fields.advisory;
      assert(typeof id === 'string' && /^RUSTSEC-\d{4}-\d{4}$/.test(id), 'Invalid RustSec ID');
      findings.add(id);
    }
  }

  assert(Number.isInteger(summary?.errors), 'Missing cargo-deny advisory summary');
  assert.equal(summary.errors, errors, 'Advisory summary and diagnostics disagree');
  assert.equal(exitCode, errors > 0 ? 1 : 0, 'Advisory diagnostics and exit code disagree');
  return [...findings].sort();
}

function selectCandidates(records, exitCode, openItems) {
  const tracked = new Set(openItems
    .filter(({ labels }) => labels.some(({ name }) => name === 'rustsec'))
    .flatMap(({ title }) => title.match(/\bRUSTSEC-\d{4}-\d{4}\b/g) ?? []));
  return readFindings(records, exitCode).filter((id) => !tracked.has(id));
}

module.exports = { readFindings, selectCandidates };
