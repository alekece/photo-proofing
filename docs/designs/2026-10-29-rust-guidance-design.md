# Repository Rust guidance

Approved in conversation on 2026-10-29.

Use root AGENTS.md as the single source of repository Rust guidance for Codex, GitHub Copilot coding agent, and Claude Code. Remove .github/instructions/rust.instructions.md after incorporating its guidance. Do not add CLAUDE.md, symlinks, generated copies, or synchronization tooling. Target Claude Code versions that support AGENTS.md directly; older versions may require a compatibility import outside this scope. Copilot PR review and IDE completion coverage are not guaranteed by this design. Start from Copilot's current policy and retain useful architecture, compatibility, and testing guidance from AGENTS.md.

Prefer established idiomatic crates when their consistency and correctness benefits justify dependency costs; explain meaningful local-implementation choices and revisit them as usage grows. Consider abstractions that clarify concepts or enforce contracts with minimal complexity, even for one implementation. Prefer imports; permit aliases and partial/full paths for disambiguation. Put documentation before other attributes.

Document the repository verification commands in AGENTS.md as a reference, without requiring every command after every edit: `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`. Explain that nightly rustfmt is required for the repository import-grouping settings. Require checks relevant to the changes before declaring completion and reporting of checks that could not run. Do not repeat formatter-controlled layout rules. Configure workspace Clippy unwrap_used and expect_used warnings, inherited by every crate, allowing both in tests. Production exceptions require a narrow allowance and an explanation of the invariant.

Approaches: choose a single AGENTS.md because the target harnesses support it directly. Maintaining duplicate policies risks drift; forwarding files, symlinks, and generation add unnecessary maintenance for the current scope. No global guidance changes, CI edits, unrelated source refactors, commits, or publication are part of implementation.

## Implementation verification

The shared AGENTS.md policy replaced duplicated Copilot guidance. All four crates inherit workspace unwrap_used and expect_used Clippy warnings, with test exceptions configured. Nightly formatting, workspace Clippy with warnings denied, and all 18 Rust tests passed. A temporary production probe verified that both lints reject violations and was removed. Independent review found no blockers. Harness-specific instruction loading was not tested. The completed implementation plan was removed before publication; this approved design is preserved.
