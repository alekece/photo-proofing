---
applyTo: "**/*.rs"
---

# Rust Style

## Types and APIs

- Encode meaningful invariants in types, favoring compile-time guarantees over runtime checks where practical. Use newtypes, enums, and typestate when they make invalid usage harder.
- Prefer typed configuration to ambiguous flags or parameters.
- Prefer methods when behavior belongs to a type; use builders when they clarify or validate construction.
- Keep visibility narrow and derive traits only when useful.
- Prefer small, widely adopted community crates over hand-written boilerplate (string conversions, `From`/`Display`-style impls, builders) when a derive covers it.
- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) checklist items relevant to the change.

## Code Organization

- Use `module.rs` and `module/child.rs`, not `mod.rs`.
- Order types, traits, implementations, and free functions logically; keep related declarations together.
- Avoid unnecessary wrapper modules, re-exports, and indirection.
- Place `use` declarations at the top of their containing module scope, including `mod tests`. Never put imports inside functions, blocks, or `impl` bodies.
- Group imports in three blank-line-separated blocks, in this order: `std`/`core`/`alloc`, external dependencies, then `crate`/`super`/`self`. Keep each block sorted.

## Naming

- Name free functions with a clear action and object (`delete_expired_sessions`, not `delete_expired`); methods may be shorter when the receiver makes the object clear.
- Use shadowing when transforming a value without changing its conceptual identity; use distinct names when both representations coexist or the difference matters.

## Ownership and Borrowing

- Prefer ownership and borrowing that make data flow explicit. Avoid unnecessary cloning, shared ownership, interior mutability, and allocations.

## Readability and Errors

- Favor readable code over cleverness. Use intermediate values and guard clauses when they help.
- Use `Option`/`Result` and iterator combinators for natural transformations; use `for`/`while` for side effects, async work, or clearer control flow.
- Separate meaningful logical steps with blank lines, especially local declarations from the processing that follows. No mechanical spacing between closely related statements.
- Propagate recoverable errors with `Result` and `?`, preserving context at abstraction boundaries. Use `unwrap`/`expect` only when an established invariant rules out failure, and prefer `expect` with a useful message.

## Comments and Docs

- Comments explain *why* (invariants, tradeoffs), not *what*.
- Document public APIs only when their contract or constraints need explanation.
