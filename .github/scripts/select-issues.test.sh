#!/usr/bin/env bash
# Offline test harness for select-issues.sh. Builds fixture directories under a
# scratch folder inside the repository tree (never /tmp) and asserts the exact
# JSON array printed by the script in SELECT_FIXTURE mode for each scenario
# from the Global Constraints / Review Focus sections of the implementation plan.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$SCRIPT_DIR/select-issues.sh"
SCRATCH="$SCRIPT_DIR/.select-issues-test-scratch"

rm -rf "$SCRATCH"
mkdir -p "$SCRATCH"
trap 'rm -rf "$SCRATCH"' EXIT

PASS=0
FAIL=0

# fixture <name> <issues.json> <blockers.json> <prs.json> <branches.txt>
fixture() {
  local dir="$SCRATCH/$1"
  mkdir -p "$dir"
  printf '%s' "$2" >"$dir/issues.json"
  printf '%s' "$3" >"$dir/blockers.json"
  printf '%s' "$4" >"$dir/prs.json"
  printf '%s' "$5" >"$dir/branches.txt"
  echo "$dir"
}

# assert_select <description> <max> <fixture-dir> <expected-json>
assert_select() {
  local desc="$1" max="$2" dir="$3" expected="$4"
  local actual
  actual=$(SELECT_FIXTURE="$dir" "$SCRIPT" "$max")
  if [[ "$actual" == "$expected" ]]; then
    echo "PASS: $desc"
    PASS=$((PASS + 1))
  else
    echo "FAIL: $desc"
    echo "  expected: $expected"
    echo "  actual:   $actual"
    FAIL=$((FAIL + 1))
  fi
}

REF1='docs/superpowers/specs/alpha-design.md'
REF2='docs/superpowers/specs/beta-design.md'

body_with_spec() {
  local branch="$1" path="$2" extra="${3:-}"
  printf '## References\n\nhttps://github.com/acme/widgets/blob/%s/%s\n\n%s\n' "$branch" "$path" "$extra"
}

# --- Scenario 1: two independent eligible issues, both selected within max ---
dir=$(fixture "two-eligible" \
  "$(jq -n --arg b1 "$(body_with_spec main "$REF1")" --arg b2 "$(body_with_spec main "$REF2")" '
    [
      {number: 1, title: "A", body: $b1, labels: []},
      {number: 2, title: "B", body: $b2, labels: []}
    ]')" \
  '{}' \
  '[]' \
  'main:docs/superpowers/specs/alpha-design.md
main:docs/superpowers/specs/beta-design.md')
assert_select "two eligible issues, both selected" 2 "$dir" '[1,2]'

# --- Scenario 2: fewer than X eligible (one eligible, max 2) ---
dir=$(fixture "fewer-than-max" \
  "$(jq -n --arg b1 "$(body_with_spec main "$REF1")" '
    [
      {number: 1, title: "A", body: $b1, labels: []}
    ]')" \
  '{}' '[]' \
  'main:docs/superpowers/specs/alpha-design.md')
assert_select "fewer than max eligible issues" 2 "$dir" '[1]'

# --- Scenario 3: zero eligible ---
# Labels use the real `gh issue list --json labels` shape: an array of label
# objects (id/name/description/color), not an array of strings.
dir=$(fixture "zero-eligible" \
  "$(jq -n --arg b1 "$(body_with_spec main "$REF1")" '
    [
      {number: 1, title: "A", body: $b1, labels: [{id: 1, name: "blocked", description: "", color: "b60205"}]}
    ]')" \
  '{}' '[]' \
  'main:docs/superpowers/specs/alpha-design.md')
assert_select "zero eligible issues yields empty array" 2 "$dir" '[]'

# --- Scenario 4: needs-attention/blocked labels excluded ---
dir=$(fixture "label-excluded" \
  "$(jq -n --arg b1 "$(body_with_spec main "$REF1")" --arg b2 "$(body_with_spec main "$REF2")" '
    [
      {number: 1, title: "A", body: $b1, labels: [{id: 2, name: "needs-attention", description: "", color: "e99695"}]},
      {number: 2, title: "B", body: $b2, labels: []}
    ]')" \
  '{}' '[]' \
  'main:docs/superpowers/specs/alpha-design.md
main:docs/superpowers/specs/beta-design.md')
assert_select "needs-attention label excludes the issue" 2 "$dir" '[2]'

# --- Scenario 5: missing spec branch/file is skipped ---
dir=$(fixture "missing-spec" \
  "$(jq -n --arg b1 "$(body_with_spec gone "$REF1")" --arg b2 "$(body_with_spec main "$REF2")" '
    [
      {number: 1, title: "A", body: $b1, labels: []},
      {number: 2, title: "B", body: $b2, labels: []}
    ]')" \
  '{}' '[]' \
  'main:docs/superpowers/specs/beta-design.md')
assert_select "unresolved spec branch/file skips the issue" 2 "$dir" '[2]'

# --- Scenario 6: issue with an open PR already is ineligible ---
dir=$(fixture "open-pr-excluded" \
  "$(jq -n --arg b1 "$(body_with_spec main "$REF1")" --arg b2 "$(body_with_spec main "$REF2")" '
    [
      {number: 1, title: "A", body: $b1, labels: []},
      {number: 2, title: "B", body: $b2, labels: []}
    ]')" \
  '{}' \
  "$(jq -n '[{number: 50, body: "Fixes #1", headRefName: "feature/1-a"}]')" \
  'main:docs/superpowers/specs/alpha-design.md
main:docs/superpowers/specs/beta-design.md')
assert_select "issue with an open linked PR is excluded" 2 "$dir" '[2]'

# --- Scenario 7: transitive blockers resolve to the base of the chain ---
dir=$(fixture "transitive-blockers" \
  "$(jq -n --arg ba "$(body_with_spec main "$REF1")" --arg bb "$(body_with_spec main "$REF1")" --arg bc "$(body_with_spec main "$REF1")" '
    [
      {number: 1, title: "A", body: $ba, labels: []},
      {number: 2, title: "B", body: $bb, labels: []},
      {number: 3, title: "C", body: $bc, labels: []}
    ]')" \
  '{"1": [2], "2": [3]}' \
  '[]' \
  'main:docs/superpowers/specs/alpha-design.md')
assert_select "transitive blocker chain resolves to the unblocked root" 2 "$dir" '[3]'

# --- Scenario 8: blocker cycle, neither issue selected ---
dir=$(fixture "blocker-cycle" \
  "$(jq -n --arg ba "$(body_with_spec main "$REF1")" --arg bb "$(body_with_spec main "$REF1")" '
    [
      {number: 1, title: "A", body: $ba, labels: []},
      {number: 2, title: "B", body: $bb, labels: []}
    ]')" \
  '{"1": [2], "2": [1]}' \
  '[]' \
  'main:docs/superpowers/specs/alpha-design.md')
assert_select "blocker cycle selects neither issue" 2 "$dir" '[]'

# --- Scenario 9: GitHub "blocked by" wins over body Dependencies text ---
dir=$(fixture "github-blockers-win" \
  "$(jq -n \
      --arg ba "$(body_with_spec main "$REF1" $'## Dependencies\n\n- #99')" \
      --arg bb "$(body_with_spec main "$REF1")" \
      --arg bc "$(body_with_spec main "$REF1")" '
    [
      {number: 20, title: "A", body: $ba, labels: []},
      {number: 21, title: "B", body: $bb, labels: []},
      {number: 99, title: "C", body: $bc, labels: [{id: 1, name: "blocked", description: "", color: "b60205"}]}
    ]')" \
  '{"20": [21]}' \
  '[]' \
  'main:docs/superpowers/specs/alpha-design.md')
assert_select "GitHub blocked-by relationship overrides body Dependencies text" 2 "$dir" '[21]'

# --- Scenario 10: body Dependencies text used only when GitHub blockers are empty ---
dir=$(fixture "body-text-fallback" \
  "$(jq -n \
      --arg ba "$(body_with_spec main "$REF1" $'## Dependencies\n\n- #30')" \
      --arg bb "$(body_with_spec main "$REF1")" '
    [
      {number: 10, title: "A", body: $ba, labels: []},
      {number: 30, title: "B", body: $bb, labels: []}
    ]')" \
  '{"10": []}' \
  '[]' \
  'main:docs/superpowers/specs/alpha-design.md')
assert_select "body Dependencies text used when GitHub blockers list is empty" 2 "$dir" '[30]'

# --- Scenario 11: a "blocked by" reference to an issue that is not open
# (closed or unknown) is not an open blocker, so the issue is unblocked ---
dir=$(fixture "closed-blocker-not-blocking" \
  "$(jq -n --arg b1 "$(body_with_spec main "$REF1")" --arg b2 "$(body_with_spec main "$REF1")" '
    [
      {number: 1, title: "A", body: $b1, labels: []},
      {number: 2, title: "B", body: $b2, labels: []}
    ]')" \
  '{"1": [5]}' \
  '[]' \
  'main:docs/superpowers/specs/alpha-design.md')
# issue 5 does not appear in the open issues list, so it is not an open
# blocker: issue 1 is unblocked and selected directly, alongside issue 2.
assert_select "blocked-by referencing a closed/unknown issue does not block" 2 "$dir" '[1,2]'

# --- Scenario 12: blockers are selected first and count toward X ---
dir=$(fixture "blockers-first-open" \
  "$(jq -n --arg b1 "$(body_with_spec main "$REF1")" --arg b2 "$(body_with_spec main "$REF1")" --arg b3 "$(body_with_spec main "$REF1")" --arg b5 "$(body_with_spec main "$REF1")" '
    [
      {number: 1, title: "A", body: $b1, labels: []},
      {number: 2, title: "B", body: $b2, labels: []},
      {number: 3, title: "C", body: $b3, labels: []},
      {number: 5, title: "E", body: $b5, labels: []}
    ]')" \
  '{"1": [5]}' \
  '[]' \
  'main:docs/superpowers/specs/alpha-design.md')
assert_select "blocker is selected in place of the blocked issue and counts toward max" 2 "$dir" '[5,2]'

# --- Scenario 13: label objects carry extra fields (node id, description,
# color) exactly as `gh issue list --json labels` returns them; only `name`
# is used for eligibility, and unrelated label objects (e.g. "agent") do not
# interfere. ---
dir=$(fixture "label-objects-extra-keys" \
  "$(jq -n --arg b1 "$(body_with_spec main "$REF1")" --arg b2 "$(body_with_spec main "$REF2")" '
    [
      {number: 1, title: "A", body: $b1, labels: [
        {id: "LA_kwDOVaqi188AAAADAA_Mmw", name: "agent", description: "Created by repository agent workflows", color: "7057ff"},
        {id: "LA_kwDOVaqi188AAAADAA_Mnw", name: "blocked", description: "Blocked on something external", color: "b60205"}
      ]},
      {number: 2, title: "B", body: $b2, labels: [
        {id: "LA_kwDOVaqi188AAAADAA_Mmw", name: "agent", description: "Created by repository agent workflows", color: "7057ff"}
      ]}
    ]')" \
  '{}' '[]' \
  'main:docs/superpowers/specs/alpha-design.md
main:docs/superpowers/specs/beta-design.md')
assert_select "label objects with extra keys (id/description/color) parse correctly" 2 "$dir" '[2]'

echo ""
echo "$PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
