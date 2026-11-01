---
name: create-issue
description: Use when the user asks to file, open, or write up a GitHub issue for the current repository from the conversation, as `create-issue` or `create-issue agent`.
---

# Create issue

Usage: `create-issue` files the issue and nothing else. `create-issue agent` also hands it to automation in the same step. Implements AGENTS.md delivery workflow step 3. `gh` targets the current checkout's repository; never pass `--repo`. Without the `agent` argument, never ask about or mention automation, and never add labels, unless the user raises it themselves (then do the Hand-off after creating).

## Steps

1. Draft from the conversation: title, motivation, observable acceptance criteria, constraints, references. Describe outcomes, not implementation task lists.
2. Check for duplicates; if one is close, show it to the user instead of creating:
   `gh issue list --state all --search "<keywords>" --limit 10`
3. Show the draft; once the user agrees, write the body to a temp file and run:
   `gh issue create --title "<title>" --body-file <file>`
   Add no labels here.
4. With the `agent` argument, run the Hand-off below on the new issue.
5. Report the issue URL (and the labels set, if any).

## Hand-off (`create-issue agent`, or the user asks for it)

No design exists yet. Create only the missing labels (show any error to the user), then set them:

```bash
gh label list --json name --jq '.[].name'
gh label create agent --description "Automation may act on this issue"                # only if missing
gh label create needs-attention --description "It is the human's turn on this issue" # only if missing
gh issue edit <N> --add-label agent --add-label needs-attention
gh issue comment <N> --body "This issue needs a design document before implementation."
```

If a label cannot be created or added, tell the user.
