---
name: create-issue
description: Use when the user asks to file, open, or write up a GitHub issue for the current repository from the conversation, including during or after brainstorming.
---

# Create issue

Usage: `create-issue`. `gh` targets the current checkout's repository; never pass `--repo`. If the issue later leads to implementation, that follows the Superpowers workflow (brainstorming -> design doc -> plan -> implementation -> review).

## Steps

1. Draft from the conversation: title, motivation, observable acceptance criteria, constraints, references. Describe outcomes, not implementation task lists. Link the design when one exists.
2. Check for duplicates; if one is close, show it to the user instead of creating:
   `gh issue list --state all --search "<keywords>" --limit 10`
3. Choose the labels and milestone:
   - One type label: `bug`, `enhancement` or `documentation`.
   - The earliest open milestone from `gh api repos/{owner}/{repo}/milestones` (MVP, then v1, and so on). An issue in the MVP milestone also gets `mvp`, except report issues.
   - `needs-attention` only when the user must act, for example the issue is incomplete or needs a design decision. Never otherwise.
4. Show the draft with its labels and milestone; once the user agrees, write the body to a temp file and run:
   `gh issue create --title "<title>" --body-file <file> --label "<labels>" --milestone "<milestone>"`
5. Report the issue URL.
