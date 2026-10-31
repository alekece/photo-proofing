# cargo deny in CI

Status: Approved in conversation on 2026-10-31 (approach A).
Issue: [#94](https://github.com/alekece/photo-proofing/issues/94).
Follows the dependency rule added to `AGENTS.md` in #92.

## Intent

Make `cargo deny` the enforced gate for dependency advisories, licenses, and duplicates, so adding or updating a dependency cannot silently introduce a known vulnerability or an unwanted license.

## Goals and non-goals

- Goals: a committed `deny.toml`, a CI job that blocks on failures, a green `main`.
- Non-goals: removing duplicate crate versions (upstream-driven, warn only), dependency or feature changes beyond the advisory fix, scheduled scans (possible later).

## Baseline (cargo-deny 0.18.9, default config)

- Advisories: fails on `h2` (RUSTSEC-2026-0258, unbounded empty DATA frames).
- Licenses: fails without an allow-list; our four crates have no `license` field because they do not inherit `[workspace.package] license = "MIT"`.
- Bans and sources: pass. Duplicate versions (`rand`, `syn`, and others) are warnings.

## Approaches

1. **`deny.toml` plus a CI job, with the fixes in the same PR (selected).** Covers all four checks and leaves `main` green.
2. Advisories only. Smaller, but drops the license and duplicate checks that were requested.
3. Scheduled job only, no PR gate. Catches new advisories but does not stop bad dependencies being merged.

## Design

- `deny.toml` at the repository root: `[licenses]` allow-list of the permissive licenses the current tree needs (derived from the actual baseline, plus MIT for our crates, nothing speculative); `[advisories]` default policy; `[bans]` `multiple-versions = "warn"`; `[sources]` crates.io only.
- Each crate in `crates/*` sets `license.workspace = true`.
- The `h2` advisory cannot be fixed by `cargo update`: `h2` 0.3.27 comes from `actix-http`, and the fix exists only in `h2` >=0.4.16. Superseding the earlier plan, `deny.toml` ignores RUSTSEC-2026-0258 with a written reason (Low severity; remove when `actix-http` moves to `h2` 0.4). Chosen by the user on 2026-10-31. No dependency changes.
- CI: a `Rust dependencies` job in `.github/workflows/ci.yml` using `EmbarkStudios/cargo-deny-action`, gated by the existing `rust` path filter like the other Rust jobs (it already includes `Cargo.lock`), running `cargo deny --workspace check`.
- `AGENTS.md`: change the "intended CI gate once configured" wording to state the gate exists.

## Left to the builder

The exact license allow-list entries (from the baseline output), the action version pin, and the exact ignore wording.

## Verification

`cargo deny --workspace check` passes locally; `cargo build --workspace --locked` and `cargo test --workspace --locked` still pass after the `h2` bump; CI shows the new job green on the PR.
