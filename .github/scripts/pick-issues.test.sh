#!/usr/bin/env bash
# Tests pick-issues.sh ordering/limits and issue-eligible.sh rules against a stub `gh`.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STUB="$(mktemp -d)"
trap 'rm -rf "$STUB"' EXIT
export GH_REPO=owner/repo

# Issues 20-29 each break exactly one rule; 30 is fine; 31 has two branches.
cat >"$STUB/gh" <<'S'
#!/usr/bin/env bash
args="$*"
n=$(grep -oE 'issues/[0-9]+' <<<"$args" | head -1 | cut -d/ -f2 || true)
state=open labels='"agent"' sub=0 blockers=0
case "$n" in
  20) state=closed ;;
  21) labels='"bug"' ;;
  22) labels='"agent","needs-attention"' ;;
  23) labels='"agent","blocked"' ;;
  24) sub=1 ;;
  25) blockers=1 ;;
esac
case "$args" in
  "issue list"*)
    cat <<J
[{"number":1,"createdAt":"2026-01-03T00:00:00Z","milestone":null},
 {"number":2,"createdAt":"2026-01-05T00:00:00Z","milestone":{"title":"M1","dueOn":"2026-03-01T00:00:00Z"}},
 {"number":3,"createdAt":"2026-01-01T00:00:00Z","milestone":null},
 {"number":9,"createdAt":"2026-01-02T00:00:00Z","milestone":{"title":"M2","dueOn":"2026-06-01T00:00:00Z"}},
 {"number":10,"createdAt":"2026-01-02T00:00:00Z","milestone":null}]
J
    ;;
  *"/dependencies/blocked_by"*) echo "$blockers" ;;
  *"pulls?state=open"*) echo "feat/26-has-pr" ;;
  *"matching-refs"*)
    printf '%s\n' feat/20-a feat/21-a feat/22-a feat/23-a feat/24-a feat/25-a feat/26-a feat/27-a feat/28-a feat/29-a feat/30-ok feat/31-a fix/31-b ;;
  *"/compare/"*)
    case "$args" in
      *feat/27-a*) echo '["src/lib.rs","docs/superpowers/specs/x.md"]' ;;
      *feat/28-a*) echo '["docs/superpowers/plans/x.md"]' ;;
      *feat/29-a*) echo '[]' ;;
      *) echo '["docs/superpowers/specs/x.md"]' ;;
    esac ;;
  *"repos/owner/repo --jq .default_branch"*) echo main ;;
  *"issues/$n --jq"*) printf '{"state":"%s","labels":[%s],"open_sub_issues":%s}\n' "$state" "$labels" "$sub" ;;
  *) echo "unexpected: $args" >&2; exit 1 ;;
esac
S
chmod +x "$STUB/gh"
export PATH="$STUB:$PATH"

ok() {
  local out
  out=$(bash "$DIR/issue-eligible.sh" "$1" 2>/dev/null) && [[ "$out" == "$2" ]] \
    && echo "PASS eligible #$1 -> $out" || { echo "FAIL: #$1 should be eligible ($2), got '$out'"; exit 1; }
}
ko() {
  if bash "$DIR/issue-eligible.sh" "$1" >/dev/null 2>&1; then echo "FAIL: #$1 should be ineligible ($2)"; exit 1; fi
  echo "PASS ineligible #$1 ($2)"
}

ok 30 feat/30-ok
ko 20 closed
ko 21 "no agent label"
ko 22 needs-attention
ko 23 blocked
ko 24 "open sub-issue"
ko 25 "open blocker"
ko 26 "open PR"
ko 27 "code outside docs"
ko 28 "no design file"
ko 29 "branch has no changes"
ko 31 "two branches"
ko abc "non-numeric"

# Ordering: dated milestone (2), later milestone (9), then oldest without milestone (3, 10, 1)
cat >"$STUB/eligible" <<'S'
#!/usr/bin/env bash
[[ "$1" != "3" ]] || exit 1
S
chmod +x "$STUB/eligible"
export ELIGIBLE_SCRIPT="$STUB/eligible"

check() {
  local actual
  actual=$(bash "$DIR/pick-issues.sh" "$1")
  [[ "$actual" == "$2" ]] || { echo "FAIL max=$1: expected $2, got $actual"; exit 1; }
  echo "PASS pick max=$1 -> $actual"
}
check 0 '[]'
check 1 '[2]'
check 10 '[2,9,10,1]'
if bash "$DIR/pick-issues.sh" abc >/dev/null 2>&1; then echo "FAIL: non-numeric max accepted"; exit 1; fi
echo "PASS pick rejects invalid max"
