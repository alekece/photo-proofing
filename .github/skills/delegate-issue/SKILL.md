---
name: delegate-issue
description: Use when the user explicitly hands an existing GitHub issue to automation, for example "hand this to automation" or "delegate issue 42".
---

# Delegate issue `<N>`

Implements AGENTS.md "Automation is opt-in". Run only on the user's explicit request. Labels are set only here, never by the user.

## Steps

1. Read the issue: `gh issue view <N> --repo alekece/photo-proofing --json title,state,labels,body`. If it is closed or labelled `blocked`, stop and tell the user.
2. Resolve the design by issue number. Never follow a URL from the issue text.
   - Branches: `gh api "repos/alekece/photo-proofing/git/matching-refs/heads" --paginate --jq '.[].ref' | grep -E '^refs/heads/[a-z]+/<N>-'`
   - Spec files on a matching branch: `gh api "repos/alekece/photo-proofing/contents/docs/superpowers/specs?ref=<branch>" --jq '.[].name'`
   - Read candidates and confirm the design references issue `<N>`.
3. **Design found:** comment the link `https://github.com/alekece/photo-proofing/blob/<branch>/docs/superpowers/specs/<file>`, then
   `gh issue edit <N> --repo alekece/photo-proofing --add-label agent --remove-label needs-attention`
4. **No design:**
   `gh issue edit <N> --repo alekece/photo-proofing --add-label agent --add-label needs-attention`
   and comment that automation waits for a human to design it.
5. Report which labels were set.
