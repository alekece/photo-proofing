---
safe-outputs:
  create-issue: false
  jobs:
    publish-rustsec-issue:
      description: Open one actionable issue for an advisory in the current scan.
      runs-on: ubuntu-latest
      needs: [safe_outputs]
      permissions:
        issues: write
      inputs:
        advisory:
          description: The exact candidate RustSec advisory ID.
          type: string
          required: true
        body:
          description: The affected dependency, remediation options and decision needed.
          type: string
          required: true
      steps:
        - name: Require a clean detector verdict for issue publication
          run: test "$GH_AW_DETECTION_CONCLUSION" = success
        - uses: github/gh-aw-actions/setup@221ba4f4319de6b8e809d1a15528b3ae6a818aa5
        - uses: actions/download-artifact@v8.0.2
          with:
            name: rustsec-scan
            path: ${{ runner.temp }}/rustsec-scan
        - uses: actions/github-script@v9.0.0
          env:
            SAFE_OUTPUTS_RESULT: ${{ needs.safe_outputs.result }}
          with:
            script: |
              const assert = require('node:assert/strict');
              const fs = require('node:fs');
              const path = require('node:path');
              const actions = path.join(process.env.RUNNER_TEMP, 'gh-aw/actions');
              const { setupGlobals, createIssue } = require(path.join(actions, 'index.cjs'));
              const { sanitizeContent } = require(path.join(actions, 'sanitize_content.cjs'));
              setupGlobals(core, github, context, exec, io, getOctokit);
              const report = JSON.parse(fs.readFileSync(path.join(process.env.RUNNER_TEMP, 'rustsec-scan/report.json')));
              const output = JSON.parse(fs.readFileSync(process.env.GH_AW_AGENT_OUTPUT, 'utf8'));
              const items = output.items.filter(item => item.type === 'publish_rustsec_issue');
              assert.equal(items.length, 1, 'Expected one issue proposal');
              const { advisory, body } = items[0];
              assert(report.sha === context.sha && report.candidate_ids.includes(advisory), 'Advisory was not selected by this scan');
              if (report.kind === 'ignore-review') {
                assert.equal(process.env.SAFE_OUTPUTS_RESULT, 'success', 'Weekly safe outputs did not complete successfully');
                assert.equal(report.findings[advisory], 'affected', 'Only affected ignores may be escalated to an issue');
              }
              assert(typeof body === 'string' && body.trim().length >= 20 && body.length <= 8000, 'Invalid issue body');
              const openItems = await github.paginate(github.rest.issues.listForRepo, {
                ...context.repo, state: 'open', labels: 'rustsec', per_page: 100,
              });
              if (openItems.some(({ title }) => title.match(/\bRUSTSEC-\d{4}-\d{4}\b/g)?.includes(advisory))) {
                core.notice('Advisory is already tracked');
                return;
              }
              const { data: [milestone] } = await github.rest.issues.listMilestones({
                ...context.repo, state: 'open', sort: 'due_on', direction: 'asc', per_page: 1,
              });
              assert(milestone, 'No open milestone for the RustSec issue');
              const labels = ['bug', 'needs-attention', 'rustsec'];
              if (milestone.title === 'MVP') labels.push('mvp');
              const result = await createIssue({
                title: `[RustSec] ${advisory}: actionable remediation`,
                body: sanitizeContent(body),
                labels,
                milestone: milestone.number,
              });
              if (!result.staged) core.info(result.issue.html_url);
---
