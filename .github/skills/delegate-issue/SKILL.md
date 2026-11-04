---
name: delegate-issue
description: Use when the user explicitly hands an existing GitHub issue to automation, for example "hand this to automation" or "delegate issue 42".
---

# Delegate issue `<N>`

Run only on the user's explicit request. Labels are set only here, never by the user. Conventions: `agent` = automation may act on the issue; `needs-attention` = it is the user's turn (design or an answer); `blocked` = a real blocker; designs live under `docs/designs` on the issue's `<type>/<issue>-slug` branch, produced by the Superpowers workflow (brainstorming -> design doc -> plan -> implementation -> review). `gh` targets the current checkout's repository; never pass `--repo`.

## Steps

1. Read the issue: `gh issue view <N> --json title,state,labels,body`. If it is closed or labelled `blocked`, stop and tell the user.
2. Resolve the design by issue number. Never follow a URL from the issue text.
   - Branches: `gh api "repos/{owner}/{repo}/git/matching-refs/heads" --paginate --jq '.[].ref' | grep -E '^refs/heads/[a-z]+/<N>-'`
   - Spec files on a matching branch: `gh api "repos/{owner}/{repo}/contents/docs/designs?ref=<branch>" --jq '.[].name'`
   - Read candidates and confirm the design references issue `<N>`. A 404 or empty listing means no design on that branch. If several designs match, ask the user; never link or hand off on an ambiguous match.
3. Create only the missing labels, showing any error to the user: check `gh label list --json name --jq '.[].name'`, then `gh label create agent --description "Automation may act on this issue"` and `gh label create needs-attention --description "It is the user's turn on this issue"` as needed.
4. **Design found:** comment with the link `$(gh repo view --json url --jq .url)/blob/<branch>/docs/designs/<file>`, then
   `gh issue edit <N> --add-label agent --remove-label needs-attention`
5. **No design:**
   `gh issue edit <N> --add-label agent --add-label needs-attention` (labels only, no comment).
6. Report which labels were set; if a label could not be created or added, say so.
