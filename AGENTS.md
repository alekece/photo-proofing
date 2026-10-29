# PhotoProofing Agent Guidance

This file is the shared repository policy for Codex, GitHub Copilot coding agent, and Claude Code. The following development principles apply to Rust changes.

## Architecture, Types, and APIs

- Consider abstractions when they give a concept a clear interface or enforce a meaningful contract, even for one implementation. Use them when they improve reasoning with little added complexity; avoid speculative extension points.
- Separate generic mechanisms from application-specific policy. Respect existing architecture; suggest worthwhile improvements without silently refactoring unrelated code.
- Encode meaningful invariants in types where practical. Use newtypes, enums, and typestate when they make invalid usage harder.
- Use traits, generics, and associated types for meaningful capabilities and cohesive contracts. Prefer static dispatch unless runtime polymorphism is warranted.
- Prefer typed configuration to ambiguous flags or parameters. Prefer methods when behavior belongs to a type; use builders when they clarify or validate construction.
- Keep visibility narrow and derive traits only when useful.
- Prefer established, idiomatic crates such as `derive_more` when they provide consistent, reliable implementations instead of handwritten boilerplate. Weigh those benefits against dependency costs, even for small uses. Explain meaningful decisions to implement locally and revisit them when additional usage changes the balance.
- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) checklist items and chapters relevant to the change. Respect repository compatibility requirements and propose breaking changes before making them.

## Working with Existing Code

- Before modifying code, inspect the relevant implementation, callers, and tests. Keep changes focused and preserve behavior outside the requested scope.

## Code Organization

- Organize code into cohesive feature or domain modules; split only when responsibilities or size justify it.
- Use `module.rs` and `module/child.rs`, not `mod.rs`.
- Order types, traits, implementations, and free functions logically; keep related declarations together.
- Avoid unnecessary wrapper modules, re-exports, and indirection.
- Place `use` declarations at the top of their containing module scope, including `mod tests`. Never put imports inside functions, blocks, or `impl` bodies.
- Prefer imports in almost all cases, referring to types by name. For disambiguation, use import aliases or partial/full paths, choosing whichever reads most clearly.
- For functions, import the parent module and call through it (`use std::fs;` then `fs::write(..)`) rather than writing the full path or importing the function itself, except when needed for disambiguation.

## Naming

- Prefer descriptive, precise names without redundant context. Use conventional short generic parameters such as `T`, `U`, `E`, `K`, and `V`.
- Name free functions and systems with a clear action and object (`delete_expired_sessions`, not `delete_expired`); methods may be shorter when the receiver makes the object clear.
- Use shadowing when transforming a value without changing its conceptual identity; use distinct names when both representations coexist or the difference matters.
- Follow Rust naming conventions and use consistent domain terminology.

## Ownership and Borrowing

- Prefer ownership and borrowing that make data flow explicit. Avoid unnecessary cloning, shared ownership, interior mutability, and allocations.

## Readability and Errors

- Favor readable code over cleverness. Use intermediate values and guard clauses when they help.
- Use `Option`/`Result` and iterator combinators for natural transformations; use `for`/`while` for side effects, async work, or clearer control flow.
- Use vertical whitespace so structure is easy to scan. Always put a blank line:
  - before and after every control-flow block (`if`, `match`, `for`, `while`, `loop`);
  - after a group of `let` declarations, before the code that uses them;
  - before `return`, unless it is the only statement in its scope.
- Beyond those rules, group remaining statements into reasonable logical blocks with blank lines, using judgment; do not space out closely related statements mechanically.
- Propagate recoverable errors with `Result` and `?`, preserving useful context at abstraction boundaries and distinguishing failures from violated invariants.
- Respect the configured Clippy lints. `unwrap` and `expect` are allowed in tests; production exceptions require a narrow lint allowance and an explanation of the established invariant. Prefer `expect` when its message adds useful context.

## Comments and Docs

- Comments explain *why* (invariants, tradeoffs), not *what*.
- Document public APIs when their purpose, contract, or constraints need explanation; add examples when helpful.
- Write documentation while implementing, not afterward, and treat it as part of "done" before requesting review or opening a PR. Reviewers should not have to reverse-engineer intent: document new public items whose contract is non-obvious (errors, invariants, security or concurrency guarantees), and give a module whose behavior spans several functions or files a short `//!` overview of the flow and where each guarantee is enforced. Add a brief *why* comment at non-obvious spots, such as deliberate tradeoffs, ordering that matters, and security-sensitive choices.
- Place documentation comments before all other attributes on the item.
- Keep documentation concise and current; avoid redundant comments and decorative headers.

## Testing and Verification

- Test new behavior, meaningful edge cases, and invariants without arbitrary coverage targets. Prefer observable contracts over implementation details.
- Use unit tests for isolated behavior and integration tests for component interactions; add regression tests for bug fixes.
- Keep tests deterministic and reuse existing infrastructure. Improve boundaries when doing so benefits design and testability.
- Verify changes with relevant formatting, linting, and tests before declaring completion. Report checks that could not run; never weaken meaningful tests merely to make code pass.

Repository command reference:

- Format: `cargo +nightly fmt --all`
- Lint: `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Test: `cargo test --workspace --locked`

Nightly rustfmt is required for the repository's import-grouping settings. Follow the repository formatter configuration; choose checks appropriate to the changes rather than running every command after each edit.

## Documentation and Development Workflow

- Start work from a product journey and a bounded user-visible slice. Existing scaffolding and historical designs do not automatically define current product scope.
- `README.md` owns the product overview, current implementation status, setup/run/verification instructions, and documentation navigation. Keep claims aligned with actual code and configuration.
- `docs/SPECIFICATIONS.md` is the authoritative, evolving product requirements document. Update it when product behavior or established constraints change; distinguish MVP restrictions, future scope, and permanent exclusions. Obtain explicit user approval before changing product requirements or release scope.
- `docs/releases/` owns release scope and acceptance criteria, referencing product requirements rather than duplicating the PRD. Record the accepted PRD commit/tag at release completion and preserve completed releases as historical records.
- `docs/adr/` records significant architectural decisions, their alternatives, and consequences. Do not create an ADR for every dependency or restate the PRD.
- Commit approved designs in `docs/superpowers/specs/` as durable records. If later requirements change, explicitly identify superseded decisions; the current PRD owns current product requirements.
- Commit implementation plans in `docs/superpowers/plans/` on the working branch and retain them through implementation, PR review, and corrections. After review and corrections, retain lasting decisions/evidence in permanent documents or the PR, then remove the relevant plans before requesting final merge approval. Permanent documents must not depend on links to temporary plan files.
- Do not add duplicate harness-specific guidance or synchronization tooling. This file is the shared repository policy.

### Superpowers delivery workflow

1. Use Superpowers brainstorming to explore the outcome, scope, and approaches. Brainstorming may begin without a GitHub issue; keep routine implementation choices proportional to the task.
2. Write a design specification and obtain explicit user approval of the written design before implementation. Approval of an approach alone does not approve the written design.
3. Before implementation planning, identify or create a focused GitHub issue describing motivation, observable acceptance criteria, constraints, and references. Issues describe outcomes rather than implementation task lists. Link the issue from the design.
4. Use Superpowers writing-plans to create an implementation plan from the approved design, referencing the design and issue.
5. Implement only the approved specification, following the plan, in an isolated worktree. Use Git Flow branch types with the issue number at the start of the descriptive name, for example `feature/61-development-workflow`. Default to implementation in the current session with one fresh reviewer at the end; task-by-task subagents are optional. If worktree creation fails, resolve the blocker rather than implementing in the main checkout.
6. Confirm the code is documented as described in Comments and Docs, run relevant checks, and request independent review by a fresh agent or subagent. Review both correctness and compliance with the approved design. Address material findings and verify corrections before delivery; if independent review is unavailable, report the blocker rather than substituting author self-review.
7. Create a PR referencing the original issue, with a summary of changes, verification evidence, and material limitations. Keep the worktree available through PR review and corrections.
8. After review and corrections, complete the plan cleanup described above and request explicit user approval of the final changes before merging. Prefer squash merges. Never merge automatically or substitute a local merge for PR delivery.

These repository rules take precedence over skill defaults that allow implementation without isolation or local merge delivery. Do not add issue templates, custom commands, or workflow automation without a concrete need and approval.
