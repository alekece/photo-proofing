#!/usr/bin/env bash
# Checks that one issue may be implemented by automation.
#
# Usage: issue-eligible.sh <issue>      (needs GH_TOKEN and GH_REPO, or a git checkout)
# Prints the design branch on success. On failure prints the reason to stderr and exits 1.
#
# Eligible: open, labelled `agent`, no `needs-attention`/`blocked`, no open "blocked by"
# blocker, no open sub-issue, no open PR for it, exactly one `<type>/<issue>-*` branch
# (the design branch) whose only changes against the default branch are under
# docs/superpowers/ and which holds a design file in docs/superpowers/specs/. Branches
# are resolved by issue number only; nothing in the issue text is followed.
set -euo pipefail

N="${1:?Usage: issue-eligible.sh <issue>}"
[[ "$N" =~ ^[0-9]+$ ]] || { echo "issue must be a number, got '$N'" >&2; exit 2; }

REPO="${GH_REPO:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}"

fail() { echo "issue #$N not eligible: $1" >&2; exit 1; }

issue=$(gh api "repos/$REPO/issues/$N" --jq '{state, labels: [.labels[].name], open_sub_issues: ((.sub_issues_summary.total // 0) - (.sub_issues_summary.completed // 0))}')

[[ "$(jq -r .state <<<"$issue")" == "open" ]] || fail "not open"
jq -e 'any(.labels[]; . == "agent")' <<<"$issue" >/dev/null || fail "missing agent label"
jq -e 'any(.labels[]; . == "needs-attention" or . == "blocked") | not' <<<"$issue" >/dev/null || fail "labelled needs-attention or blocked"
[[ "$(jq -r .open_sub_issues <<<"$issue")" -eq 0 ]] || fail "has open sub-issues"

open_blockers=$(gh api "repos/$REPO/issues/$N/dependencies/blocked_by" --jq '[.[] | select(.state == "open")] | length')
[[ "$open_blockers" -eq 0 ]] || fail "has open blockers"

pr_heads=$(gh api "repos/$REPO/pulls?state=open&per_page=100" --paginate --jq '.[].head.ref')
if grep -Eq "^[a-z]+/$N-" <<<"$pr_heads"; then
  fail "has an open PR"
fi

branches=$(gh api "repos/$REPO/git/matching-refs/heads" --paginate --jq '.[].ref | sub("^refs/heads/"; "")' | grep -E "^[a-z]+/$N-" || true)
[[ -n "$branches" ]] || fail "no design branch"
[[ "$(wc -l <<<"$branches")" -eq 1 ]] || fail "more than one branch exists for it"
branch="$branches"

default_branch=$(gh api "repos/$REPO" --jq .default_branch)
changed=$(gh api "repos/$REPO/compare/$default_branch...$branch" --jq '[.files[].filename]')
jq -e 'length > 0 and all(.[]; startswith("docs/superpowers/"))' <<<"$changed" >/dev/null || fail "branch has changes outside docs/superpowers/"
jq -e 'any(.[]; startswith("docs/superpowers/specs/") and endswith(".md"))' <<<"$changed" >/dev/null || fail "no design file on branch"

echo "$branch"
