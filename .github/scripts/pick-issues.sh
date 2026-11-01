#!/usr/bin/env bash
# Prints a JSON array with up to <max> issue numbers that implement-issue may pick up.
#
# Usage: pick-issues.sh <max>        (needs GH_TOKEN and GH_REPO, or a git checkout)
#
# Eligibility is decided per issue by issue-eligible.sh (set ELIGIBLE_SCRIPT to override
# in tests).
#
# Order: issues in an open milestone first (earliest due date, then title), then the
# rest; ties are broken by oldest issue first. Project-board priority is not
# supported: GITHUB_TOKEN cannot read Projects.
set -euo pipefail

MAX="${1:?Usage: pick-issues.sh <max>}"

[[ "$MAX" =~ ^[0-9]+$ ]] || { echo "max must be a non-negative integer, got '$MAX'" >&2; exit 2; }
[[ "$MAX" -gt 0 ]] || { echo "[]"; exit 0; }

REPO="${GH_REPO:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}"

candidates=$(gh issue list --repo "$REPO" --state open --label agent --limit 200 \
  --json number,createdAt,milestone)
eligible="[]"

while IFS= read -r issue; do
  n=$(jq -r '.number' <<<"$issue")

  if "${ELIGIBLE_SCRIPT:-$(dirname "${BASH_SOURCE[0]}")/issue-eligible.sh}" "$n" >/dev/null 2>&1; then
    eligible=$(jq -c --argjson i "$issue" '. + [$i]' <<<"$eligible")
  fi
done < <(jq -c '.[]' <<<"$candidates")

# Milestones without a due date sort after dated ones.
jq -c --argjson max "$MAX" '
  sort_by([
    (if .milestone and .milestone.dueOn then 0 elif .milestone then 1 else 2 end),
    (.milestone.dueOn // ""),
    (.milestone.title // ""),
    .createdAt
  ]) | map(.number) | .[:$max]' <<<"$eligible"
