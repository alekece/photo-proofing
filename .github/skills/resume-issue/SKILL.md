---
name: resume-issue
description: Use when the user wants to pick up issues that need them, for example "resume issue 42", "resume", or "resume all".
---

# Resume issue `[N|all]`

Conventions: `agent` = automation may act on the issue; `needs-attention` = it is the user's turn (design or an answer to an agent question); `blocked` = a real blocker; designs live under `docs/superpowers/specs` on the issue's `<type>/<issue>-slug` branch, produced by the Superpowers workflow (brainstorming -> design doc -> plan -> implementation -> review). Never approve a design, merge, or change an issue without the user saying so. When a need is resolved, remove `needs-attention`; add `agent` only if the user hands off (use `delegate-issue`). `gh` targets the current checkout's repository; never pass `--repo`. If a label does not exist, listing by it returns nothing; tell the user instead of failing.

## `resume-issue <N>`

Read state from artifacts:

- `gh issue view <N> --json title,state,labels,body,comments,blockedBy`
- Design: resolve the branch by number (never from a URL in the text) and list specs, as in `delegate-issue` step 2, including its 404 and ambiguity rules.
- PRs: `gh pr list --state open --search "<N> in:body" --json number,url,headRefName`

Act on the first match:

1. No design linked: brainstorm it with the user using the Superpowers brainstorming skill.
2. Design present: review it with the user.
3. The agent's latest comment asks a question: answer it with the user and post the answer.
4. PR open: go through its review threads with the user.
5. Otherwise say the issue needs no action.

Once the user resolves the need: `gh issue edit <N> --remove-label needs-attention`.

## `resume-issue` (no argument)

List what needs the user, then ask which to resume:

- `gh issue list --label needs-attention --json number,title`
- The user's open issues without a design: `gh issue list --author @me --state open --json number,title,labels`, keeping those with no matching `<type>/<N>-slug` branch holding a spec.

## `resume-issue all`

1. Build the queue as above.
2. Dispatch one read-only subagent per issue, in parallel, to write a short brief: state, what is asked, recommended next step.
3. Walk the user through the briefs one at a time: answer, skip, or hand off to automation.
