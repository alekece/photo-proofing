# Superpowers development workflow

Status: Approved in conversation on 2026-10-29.
Issue: [#61](https://github.com/alekece/photo-proofing/issues/61).

## Intent

Make the repository's existing shared guidance describe a simple, repeatable development workflow, keeping implementation tied to approved outcomes and preserving human control over merging.

## Goals and constraints

Extend AGENTS.md and its README contribution summary. Preserve existing Rust guidance and document ownership. Product code, requirements, release scope, installed skills, and global configuration are outside this change. No new dependencies, issue templates, scripts, custom commands, automation, or duplicate harness policy files.

## Approaches

The user selected extending AGENTS.md and the README summary. A separate workflow policy would duplicate the existing shared guidance. Adding an issue template could improve consistency, but existing Superpowers brainstorming is sufficient; defer templates until a concrete need appears.

## Approved workflow

1. Start with a bounded outcome. Brainstorming may begin without a GitHub issue.
2. Use Superpowers brainstorming to explore approaches and write a design specification. Obtain explicit user approval of the written design before implementation.
3. Before implementation planning, create or identify a GitHub issue describing motivation, observable acceptance criteria, constraints, and references. Link the issue from the design. Issues describe outcomes rather than implementation task lists.
4. Use Superpowers to create an implementation plan from the approved design.
5. Implement only the approved specification in an isolated worktree, following the plan. Use Git Flow branch types with the issue number at the start of the descriptive name, for example feature/61-development-workflow. Default to implementation in the current session with one fresh reviewer at the end; task-by-task subagents remain optional. Do not fall back to implementation in the main checkout if isolation fails.
6. Run relevant checks, then request independent review by a fresh agent or subagent. Review both correctness and compliance with the approved design. Address findings and verify corrections.
7. Create a PR referencing the issue, including verification evidence and material limitations. Preserve the worktree for corrections.
8. After review and corrections, retain lasting decisions and evidence in permanent documentation or the PR, remove the relevant plans, and request explicit approval of the final changes before merging. Prefer squash merges. Never merge automatically.

## Documentation lifecycle

Approved designs are durable, committed records in docs/superpowers/specs/. Commit working plans under docs/superpowers/plans/ on the branch and retain them through implementation, PR review, and corrections. Remove them before final merge approval; permanent documents must not depend on temporary plan links.

Use docs/adr/ for significant architectural decisions, not routine implementation details. docs/SPECIFICATIONS.md remains the product authority, and docs/releases/ owns release scope. Obtain approval before changing product requirements or release scope. Identify superseded design decisions when requirements change.

## Acceptance and builder discretion

AGENTS.md states the sequence, approval gates, isolation, review, traceability, and lifecycle rules without duplicating installed skills. README summarizes the workflow and links to shared guidance. Check local links, whitespace, design coverage, and preservation of unrelated content; application tests are unnecessary for prose-only changes.

Exact prose and formatting are left to the implementer. Repository rules override skill defaults permitting implementation without isolation or local merge delivery. Guidance is not GitHub branch-protection enforcement.
