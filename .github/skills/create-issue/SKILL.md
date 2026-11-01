---
name: create-issue
description: Use when the user asks to file, open, or write up a GitHub issue for this repository from the current conversation.
---

# Create issue

Implements AGENTS.md delivery workflow step 3. **Never ask the user about automation.** Handle it only under "Hand-off" when the user raises it.

## Steps

1. Draft from the conversation: title, motivation, observable acceptance criteria, constraints, references. Describe outcomes, not implementation task lists.
2. Check for duplicates; if one is close, show it to the user instead of creating:
   `gh issue list --repo alekece/photo-proofing --state all --search "<keywords>" --limit 10`
3. Show the draft; once the user agrees, write the body to a temp file and run:
   `gh issue create --repo alekece/photo-proofing --title "<title>" --body-file <file>`
   Add no labels.
4. Report the issue URL.

## Hand-off (only if the user says to hand it to automation)

No design exists yet, so the issue waits for a human:

```bash
gh issue edit <N> --repo alekece/photo-proofing --add-label agent --add-label needs-attention
gh issue comment <N> --repo alekece/photo-proofing --body "Needs a design from a human before automation can implement this."
```
