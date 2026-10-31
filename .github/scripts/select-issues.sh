#!/usr/bin/env bash
# Selects up to <max> eligible issue numbers for daily-issue-selection.md.
#
# Usage: select-issues.sh <max>
# Prints a JSON array of issue numbers (length <= max) to stdout.
#
# Live mode calls `gh` against the current repository (requires GH_TOKEN).
# Offline/test mode is selected by setting SELECT_FIXTURE=<dir>, which must
# contain issues.json, blockers.json, prs.json, and branches.txt (see
# select-issues.test.sh for the exact shapes).
#
# Eligibility (docs/superpowers/plans/2026-10-31-daily-issue-implementation.md):
#   1. Issue has neither `needs-attention` nor `blocked` label.
#   2. Its body References link an existing design spec file on a branch
#      (docs/superpowers/specs/*-design.md); the file must resolve.
#   3. No open blocker: GitHub's "blocked by" relationship wins over a body
#      `## Dependencies` section when both are present; resolution is
#      transitive. A blocked issue is skipped unless every blocker in its
#      chain is itself eligible, in which case the blocker is selected
#      instead (and counts toward <max>).
#   4. It has no open linked PR already (by "closes #N" style reference in a
#      PR body, or a `feature/<n>-*` head branch).
set -euo pipefail

MAX="${1:?Usage: select-issues.sh <max>}"

if [[ -n "${SELECT_FIXTURE:-}" ]]; then
  FIXTURE_DIR="$SELECT_FIXTURE"
  issues_json=$(cat "$FIXTURE_DIR/issues.json")
  blockers_json=$(cat "$FIXTURE_DIR/blockers.json" 2>/dev/null || echo '{}')
  prs_json=$(cat "$FIXTURE_DIR/prs.json" 2>/dev/null || echo '[]')
  branches_file="$FIXTURE_DIR/branches.txt"
else
  : "${GH_TOKEN:?GH_TOKEN is required outside SELECT_FIXTURE mode}"
  REPO="${GH_REPO:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}"
  issues_json=$(gh issue list --repo "$REPO" --state open --json number,title,body,labels --limit 500)
  prs_json=$(gh pr list --repo "$REPO" --state open --json number,body,headRefName --limit 500)
  blockers_json="{}"
  while IFS= read -r n; do
    blocked=$(gh api "repos/$REPO/issues/$n/dependencies/blocked_by" --jq '[.[].number]' 2>/dev/null || echo '[]')
    blockers_json=$(jq --argjson b "$blocked" --arg n "$n" '. + {($n): $b}' <<<"$blockers_json")
  done < <(jq -r '.[].number' <<<"$issues_json")
fi

# --- Spec link extraction and resolution -----------------------------------

# Extracts the first "<branch>:<path>" spec reference from an issue body,
# accepting either a full blob URL (.../blob/<branch>/docs/.../*-design.md)
# or the shorthand "<branch>:docs/.../*-design.md". Prints nothing when no
# reference is found.
extract_spec_link() {
  local body="$1" link
  link=$(printf '%s' "$body" | grep -oP '(?<=/blob/)[^[:space:])]+/docs/superpowers/specs/[A-Za-z0-9_.-]+-design\.md' | head -n1 || true)
  if [[ -n "$link" ]]; then
    printf '%s' "${link/\/docs\//:docs/}"
    return
  fi

  link=$(printf '%s' "$body" | grep -oP '[A-Za-z0-9/_.-]+:docs/superpowers/specs/[A-Za-z0-9_.-]+-design\.md' | head -n1 || true)
  printf '%s' "$link"
}

# Returns success when <branch>:<path> resolves (branch exists and the file
# is present on it).
spec_resolves() {
  local branch="$1" path="$2"

  if [[ -n "${SELECT_FIXTURE:-}" ]]; then
    [[ -f "$branches_file" ]] && grep -qxF "$branch:$path" "$branches_file"
    return
  fi

  gh api "repos/$REPO/branches/$branch" >/dev/null 2>&1 || return 1
  gh api "repos/$REPO/contents/$path?ref=$branch" >/dev/null 2>&1
}

# --- Body "## Dependencies" fallback parsing --------------------------------

# Extracts issue numbers (deduplicated, ascending) listed under a
# "## Dependencies" (or deeper heading level) section of the body, used only
# when GitHub reports no "blocked by" relationships for the issue.
extract_body_blockers() {
  local body="$1"
  printf '%s' "$body" | awk '
    BEGIN { insec = 0 }
    /^#+[ \t]*Dependencies[ \t]*$/ { insec = 1; next }
    /^#+/ { if (insec) exit }
    insec { print }
  ' | { grep -oE '#[0-9]+' || true; } | tr -d '#' | sort -un
}

# --- Load per-issue facts ----------------------------------------------------

declare -A ISSUE_EXISTS
declare -A LABEL_BLOCKED
declare -A SPEC_OK
declare -A HAS_OPEN_PR
declare -A BLOCKERS

mapfile -t ISSUE_NUMBERS < <(jq -r '.[].number' <<<"$issues_json" | sort -n)

for n in "${ISSUE_NUMBERS[@]}"; do
  ISSUE_EXISTS[$n]=1
done

while IFS= read -r issue_line; do
  n=$(jq -r '.number' <<<"$issue_line")
  body=$(jq -r '.body // ""' <<<"$issue_line")
  labels=$(jq -r '[(.labels // [])[] | if type == "object" then .name else . end] | join(",")' <<<"$issue_line")

  if [[ ",$labels," == *",needs-attention,"* || ",$labels," == *",blocked,"* ]]; then
    LABEL_BLOCKED[$n]=1
  else
    LABEL_BLOCKED[$n]=0
  fi

  link=$(extract_spec_link "$body")
  if [[ -n "$link" ]] && spec_resolves "${link%%:*}" "${link#*:}"; then
    SPEC_OK[$n]=1
  else
    SPEC_OK[$n]=0
  fi

  gh_list=$(jq -c --arg n "$n" '.[$n] // []' <<<"$blockers_json")
  if [[ "$(jq 'length' <<<"$gh_list")" -gt 0 ]]; then
    blocker_candidates=$(jq -r '.[]' <<<"$gh_list")
  else
    blocker_candidates=$(extract_body_blockers "$body")
  fi

  filtered=""
  for b in $blocker_candidates; do
    if [[ "${ISSUE_EXISTS[$b]:-0}" == "1" && "$b" != "$n" ]]; then
      filtered="$filtered $b"
    fi
  done
  BLOCKERS[$n]=$(printf '%s\n' $filtered | awk 'NF' | sort -un | tr '\n' ' ')
done < <(jq -c '.[]' <<<"$issues_json")

while IFS= read -r pr_line; do
  pr_body=$(jq -r '.body // ""' <<<"$pr_line")
  head_ref=$(jq -r '.headRefName // ""' <<<"$pr_line")

  while IFS= read -r closed_num; do
    [[ -n "$closed_num" ]] && HAS_OPEN_PR[$closed_num]=1
  done < <(printf '%s' "$pr_body" | { grep -oiE '(close[sd]?|fix(es|ed)?|resolve[sd]?)[[:space:]]+#[0-9]+' || true; } | { grep -oE '[0-9]+' || true; })

  if [[ "$head_ref" =~ ^feature/([0-9]+)- ]]; then
    HAS_OPEN_PR[${BASH_REMATCH[1]}]=1
  fi
done < <(jq -c '.[]' <<<"$prs_json")

# --- Eligibility and blocker-chain resolution --------------------------------

is_base_eligible() {
  local n="$1"
  [[ "${LABEL_BLOCKED[$n]:-0}" == "0" ]] || return 1
  [[ "${SPEC_OK[$n]:-0}" == "1" ]] || return 1
  [[ "${HAS_OPEN_PR[$n]:-0}" == "0" ]] || return 1
}

declare -A RESOLVE_CACHE
REPLY=""

# Resolves issue <n> to the set of issue numbers that should actually be
# dispatched for it: itself when base-eligible with no open blockers, or the
# (transitively) resolved blockers when every blocker in the chain also
# resolves. Sets the global REPLY to a space-separated list, or "FAIL" when
# the issue is ineligible, has an unresolved blocker, or sits in a cycle.
# <stack> is a colon-delimited set of ancestors used for cycle detection.
resolve_issue() {
  local n="$1" stack="$2"

  if [[ "$stack" == *":$n:"* ]]; then
    REPLY="FAIL"
    return
  fi

  if [[ -n "${RESOLVE_CACHE[$n]+set}" ]]; then
    REPLY="${RESOLVE_CACHE[$n]}"
    return
  fi

  if [[ "${ISSUE_EXISTS[$n]:-0}" != "1" ]] || ! is_base_eligible "$n"; then
    RESOLVE_CACHE[$n]="FAIL"
    REPLY="FAIL"
    return
  fi

  local blockers="${BLOCKERS[$n]:-}"
  if [[ -z "${blockers// /}" ]]; then
    RESOLVE_CACHE[$n]="$n"
    REPLY="$n"
    return
  fi

  local result="" b new_stack="${stack}${n}:"
  for b in $blockers; do
    resolve_issue "$b" "$new_stack"
    if [[ "$REPLY" == "FAIL" ]]; then
      RESOLVE_CACHE[$n]="FAIL"
      REPLY="FAIL"
      return
    fi
    result="$result $REPLY"
  done

  result=$(printf '%s\n' $result | awk 'NF' | sort -un | tr '\n' ' ')
  RESOLVE_CACHE[$n]="$result"
  REPLY="$result"
}

# --- Select up to MAX issue numbers, blockers first --------------------------

SELECTED=()

is_selected() {
  local x
  for x in "${SELECTED[@]:-}"; do
    [[ "$x" == "$1" ]] && return 0
  done
  return 1
}

for n in "${ISSUE_NUMBERS[@]}"; do
  [[ "${#SELECTED[@]}" -ge "$MAX" ]] && break
  is_selected "$n" && continue

  resolve_issue "$n" ":"
  [[ "$REPLY" == "FAIL" ]] && continue

  for t in $REPLY; do
    [[ "${#SELECTED[@]}" -ge "$MAX" ]] && break
    is_selected "$t" || SELECTED+=("$t")
  done
done

if [[ "${#SELECTED[@]}" -eq 0 ]]; then
  echo "[]"
else
  printf '%s\n' "${SELECTED[@]}" | jq -c -R 'tonumber' | jq -cs '.'
fi
