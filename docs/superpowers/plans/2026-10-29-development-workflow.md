# Development Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the approved Superpowers delivery workflow explicit in shared repository documentation.

**Architecture:** Extend the existing AGENTS.md policy and README summary. Preserve document ownership and existing Rust guidance; add no tooling.

**Tech Stack:** Markdown, Git, GitHub, existing Superpowers skills.

**Spec:** docs/superpowers/specs/2026-10-29-development-workflow-design.md

**Issue:** https://github.com/alekece/photo-proofing/issues/61

## Global Constraints

- Work in .worktrees/61-development-workflow on feature/61-development-workflow.
- Change only AGENTS.md, README.md, this working plan, and the approved design record.
- No product, release, dependency, installed-skill, or global-configuration changes.
- Keep the plan through PR review and corrections; remove it before requesting final merge approval.
- Never merge automatically.

## Review Focus

- Brainstorming without an issue remains allowed; issue creation precedes implementation planning.
- Explicit written-design approval cannot be replaced by approval of an approach.
- Isolation failures cannot silently trigger implementation in the main checkout.
- Independent review covers both correctness and design compliance, including corrections.
- Plan cleanup happens after review/corrections and before final merge approval; permanent records remain useful without the plan.

## Task 1: Document and review the workflow

**Files:** Modify AGENTS.md and README.md; preserve the approved design above.

**Interfaces:** Consumes the approved design; produces shared agent policy and its contributor summary.

- [ ] Read the approved design and existing workflow section; confirm the missing rules against the five review conditions above.
- [ ] Extend AGENTS.md with the approved sequence, issue timing/links, explicit approval gates, Git Flow branch types with issue-prefixed names (for example feature/61-development-workflow), session implementation default, isolated worktree requirement, fresh review, PR delivery, and plan lifecycle. Preserve existing Rust policy and document ownership.
- [ ] Replace the README contribution summary with a concise workflow overview linking the shared policy. Avoid restating the full rules.
- [ ] Verify with git diff --check and inspect the complete diff. Check Markdown local links, each approved workflow requirement, and unchanged product/release documents. Expected: clean whitespace, valid links, complete scope, no unrelated changes. Do not add prose tests or run application suites for this documentation-only change.
- [ ] Commit the approved design, plan, and policy changes on the issue branch. Request a fresh read-only review of the full branch against the design and five review conditions; address material findings and repeat relevant checks.
- [ ] Push and create a PR referencing #61 with verification evidence. Keep this plan through review and corrections. Do not merge; remove the plan only at the pre-merge stage before requesting final approval.
