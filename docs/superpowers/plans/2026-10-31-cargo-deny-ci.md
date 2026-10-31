# cargo deny in CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce `cargo deny` in CI with a green baseline.

**Architecture:** A root `deny.toml`, workspace license inheritance, a patched `h2`, and one CI job behind the existing `rust` path filter.

**Tech Stack:** cargo-deny 0.18.9, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-31-cargo-deny-ci-design.md` (issue #94)

## Global Constraints

- No dependency changes other than `cargo update -p h2`.
- Duplicate crate versions warn only; sources are crates.io only.
- License allow-list holds only licenses the current tree needs, plus MIT.

## Review Focus

- The job must be skipped (counted as passing) when the `rust` filter is false, like the other Rust jobs.
- The allow-list must not be broader than the baseline requires.
- `cargo update -p h2` must not move unrelated packages in `Cargo.lock`.

---

### Task 1: Make the baseline green

**Files:** Create `deny.toml`; Modify `crates/*/Cargo.toml`, `Cargo.lock`

- [ ] **Step 1:** Add `license.workspace = true` to `[package]` in the four crates.
- [ ] **Step 2:** Run `cargo update -p h2`; check `git diff --stat Cargo.lock` touches only `h2` (and its necessary dependencies).
- [ ] **Step 3:** Create `deny.toml` per the design, starting from `cargo deny init` output; set the license allow-list from `cargo deny list` output.
- [ ] **Step 4:** Run `cargo deny --workspace check`. Expected: `advisories ok, bans ok, licenses ok, sources ok`.
- [ ] **Step 5:** Run `cargo build --workspace --locked` and `cargo test --workspace --locked`. Expected: pass.
- [ ] **Step 6:** Commit `build: add cargo deny configuration and patch h2 (#94)`.

### Task 2: CI job and guidance wording

**Files:** Modify `.github/workflows/ci.yml`, `AGENTS.md`

- [ ] **Step 1:** Add job `Rust dependencies` running `cargo deny --workspace check` through `EmbarkStudios/cargo-deny-action`, with the same `needs: changes` and `if: needs.changes.outputs.rust == 'true'` condition as the other Rust jobs; add `deny.toml` to the `rust` path filter.
- [ ] **Step 2:** Change the `AGENTS.md` wording from "intended CI gate once configured" to state that `cargo deny` gates CI.
- [ ] **Step 3:** Validate YAML with `python -c "import yaml;yaml.safe_load(open('.github/workflows/ci.yml'))"`. Expected: no error.
- [ ] **Step 4:** Commit `ci: run cargo deny (#94)`.

### Task 3: Review and delivery

- [ ] **Step 1:** Fresh Opus reviewer against the design and Review Focus; fix material findings.
- [ ] **Step 2:** Open PR (app token, assign @alekece, `Closes #94`); confirm the new job is green.
- [ ] **Step 3:** Remove this plan before requesting merge approval.
