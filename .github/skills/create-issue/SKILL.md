---
name: create-issue
description: Use when the user asks to file, open, or write up a GitHub issue for the current repository from the conversation.
---

# Create issue

Usage: `create-issue`. Implements AGENTS.md delivery workflow step 3. `gh` targets the current checkout's repository; never pass `--repo`. This skill never touches the `agent` or `needs-attention` labels and never raises automation itself.

## Steps

1. Draft from the conversation: title, motivation, observable acceptance criteria, constraints, references. Describe outcomes, not implementation task lists.
2. Check for duplicates; if one is close, show it to the user instead of creating:
   `gh issue list --state all --search "<keywords>" --limit 10`
3. Show the draft; once the user agrees, write the body to a temp file and run:
   `gh issue create --title "<title>" --body-file <file>`
   Add no labels.
4. Report the issue URL.
5. Only if the user said in the conversation to delegate it (for example "create it and delegate"), invoke the `delegate-issue` skill on the new issue number.
