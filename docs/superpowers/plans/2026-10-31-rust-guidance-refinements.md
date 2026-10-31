# Rust Guidance Refinements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record the approved Rust rules in `AGENTS.md`.

**Architecture:** Documentation-only change. Each rule is one bullet added to the existing `AGENTS.md` section it belongs to, plus one new section for async and concurrency. No source, CI, or dependency changes.

**Tech Stack:** Markdown, git, `gh`.

**Spec:** `docs/superpowers/specs/2026-10-31-rust-guidance-refinements-design.md` (issue #92)

## Global Constraints

- Edit `AGENTS.md` only (plus committing the design and this plan). No source refactoring, no `Cargo.toml` or CI edits.
- One bullet per rule, matching the file's existing style: terse, imperative, no decorative headers.
- `thiserror` is the standard for error types; `derive_more` stays for non-error boilerplate. The migration of existing `derive_more` error types is a follow-up, so word the rule as the standard for new and touched code.
- `cargo deny` is not configured yet. Do not claim CI enforces it; state it as the intended gate, with its setup a follow-up.
- Do not add rules for unsafe, FFI, `no_std`, Cargo features, or semver/public-API stability.
- Do not change product scope; the separate image worker and PostgreSQL queue stay as specified.
- No book text is reproduced.

## Review Focus

- A rule that contradicts an existing bullet, most likely "common traits on public types" against "derive traits only when useful" and the secret `Debug` rule.
- The dependency rule duplicating the existing `derive_more`/established-crates bullet instead of extending it.
- Claiming `cargo deny` or the `thiserror` migration is already in place when the repository does not yet have it.
- Rules phrased so strictly that existing code violates them with no stated follow-up (for example `derive_more::Error` types in `core`, `database`, `api`).
- A new section placed so that the Superpowers workflow numbering or the command reference is disturbed.

---

### Task 1: Isolated branch with design and plan

**Files:**
- Create: `docs/superpowers/specs/2026-10-31-rust-guidance-refinements-design.md` (move from the main checkout)
- Create: `docs/superpowers/plans/2026-10-31-rust-guidance-refinements.md` (move from the main checkout)

**Interfaces:**
- Produces: a worktree on branch `feature/92-rust-guidance-refinements`, branched from `main`, containing the committed design and plan.

- [ ] **Step 1: Create the worktree** with the `superpowers:using-git-worktrees` skill, branch `feature/92-rust-guidance-refinements` from `main`.
- [ ] **Step 2: Move the two untracked files** from the main checkout into the same paths in the worktree, so the main checkout is left without them.
- [ ] **Step 3: Verify** with `git status --short` in the worktree: exactly those two files are untracked, and in the main checkout neither remains.
- [ ] **Step 4: Commit** both files with message `docs: add Rust guidance refinements design and plan (#92)`.

### Task 2: Add the rules to `AGENTS.md`

**Files:**
- Modify: `AGENTS.md` (sections "Architecture, Types, and APIs", "Readability and Errors", "Comments and Docs", and a new "Async and Concurrency" section between "Ownership and Borrowing" and "Readability and Errors")

**Interfaces:**
- Consumes: the rule list in the design's "Rules to add" section.
- Produces: `AGENTS.md` containing each rule once, in the section named below.

- [ ] **Step 1: Errors rules** in "Readability and Errors", extending the existing `Result`/`?` bullet without duplicating it. Cover: typed error enums per crate or domain boundary; only `api` maps errors to HTTP and never sends internal causes to clients, using the shared `ErrorResponse` shape; infrastructure causes wrapped as `source` and not exposed as public variants; `thiserror` for error types with `derive_more` kept for other boilerplate; `#[non_exhaustive]` only for intentionally published types.
- [ ] **Step 2: New "Async and Concurrency" section** with these rules: no blocking in async code (CPU-bound work through `spawn_blocking` or a bounded pool); multi-step state changes in one database transaction; every spawned task has an owner (joined or cancelled by a shared shutdown signal); in-memory state limited to cheap-to-lose data with PostgreSQL authoritative; `std::sync::Mutex` for short sections and no guard across `.await`; bounded channels, with slow consumers lagged or dropped.
- [ ] **Step 3: Secrets and observability** in "Readability and Errors": credential and bearer-secret newtypes (share links included) never derive `Debug`, omit it or redact with a doc note; secrets never in logs or spans, `#[instrument]` skips sensitive arguments; every request or job span carries an identifier.
- [ ] **Step 4: Interfaces and docs** in "Architecture, Types, and APIs" and "Comments and Docs": `#[must_use]` applied as a principle to builders, guards and results harmful to drop; new public items with non-obvious contracts get a short runnable doc example; common traits on public types where sound, except credentials, worded so it does not contradict "derive traits only when useful".
- [ ] **Step 5: Dependencies and macros** in "Architecture, Types, and APIs", extending the existing established-crates bullet instead of adding a duplicate: prefer established crates even for small needs and write our own only when the crate is unmaintained, heavy, or the need is trivial; declare in `[workspace.dependencies]` with minimal features; `cargo deny` is the intended gate once configured. Macros: no `macro_rules!` without a stated reason and doc example, declarative derives that generate companion types are welcome after checking for an existing crate, custom proc-macros need their own crate and an ADR.
- [ ] **Step 6: Verify the file.**
  - Run `git diff --stat`: only `AGENTS.md` changed.
  - Run `grep -c 'thiserror\|spawn_blocking\|must_use\|cargo deny\|macro_rules\|ErrorResponse' AGENTS.md` and confirm each term appears.
  - Run `grep -n 'unsafe\|no_std\|non_exhaustive' AGENTS.md` and confirm the only match is `non_exhaustive` in the errors rule and nothing about unsafe policy was added.
  - Read the diff once for contradictions listed in Review Focus.
- [ ] **Step 7: Commit** with message `docs: refine Rust guidance in AGENTS.md (#92)`.

### Task 3: Review and delivery

**Interfaces:**
- Consumes: the branch from Tasks 1–2.
- Produces: a PR referencing #92.

- [ ] **Step 1: Request independent review** from a fresh Opus subagent against the design: each rule present once, nothing outside the non-goals added, no contradictions, and no claim of unconfigured tooling. Address material findings.
- [ ] **Step 2: Open the PR** with `GH_TOKEN=$(~/.agents/bin/gh-app-token) gh pr create`, assigning @alekece, linking `Closes #92`, summarizing changes, noting that no code checks apply to a documentation-only change, and listing the follow-ups (thiserror migration, `cargo deny` setup).
- [ ] **Step 3: Before requesting merge approval**, remove `docs/superpowers/plans/2026-10-31-rust-guidance-refinements.md` from the branch, as `AGENTS.md` requires.
