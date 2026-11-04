# Rust Guidance (`crates/`)

These rules apply to Rust changes. They add to the repository-wide policy in the root `AGENTS.md`; on Rust-specific questions this file wins. Repository-specific technology choices are recorded in `docs/adr/`.

## Architecture, Types, and APIs

- Add an abstraction (trait, wrapper, module boundary) when it gives a concept a clear interface or enforces a contract that callers would otherwise have to remember. Do not add extension points for hypothetical future needs.
- Keep generic mechanisms separate from application-specific policy. Do not refactor unrelated code; if you see a worthwhile improvement outside the task, mention it in your final message or the PR description instead of making it.
- Encode invariants in types: use newtypes, enums, or typestate when they make an invalid state or call unrepresentable. Do not add a newtype that encodes no invariant; use the existing type (`NonZeroU64`, `Duration`) instead.
- Never use a primitive with a unit suffix in a field or parameter name (`timeout_ms`, `size_bytes`, `price_cents`). Use a type that carries the unit and its valid range (`Duration`, `Url`, a bounded newtype). Convert to a primitive only at a serialization or foreign-interface boundary. Parse into a validated type once at the boundary instead of calling `validate()` repeatedly.
- Use traits, generics, and associated types for capabilities that several types share. Use static dispatch unless you need runtime polymorphism (a heterogeneous collection or a plugin boundary).
- Replace boolean or stringly-typed flags with enums or config types. Use a builder when construction has many optional parts or needs validation.
- Put behavior on the type that owns the data; avoid free functions. Before writing one, use the first that fits:
  - inline it when it has a single short call site;
  - make it a method or associated function of the type it works on, introducing a small type when the logic has no owner yet (`Renderer`, `Listener<T>`, `StreamSlots`);
  - express it as a standard trait (`From`, `TryFrom`, `FromStr`, `Default`) or an extension trait;
  - in `api`, use the actix mechanism: an extractor (`FromRequest`), a middleware, or a `#[get]`/`#[post]` handler registered with `.service`.
  A free function remains acceptable for a private test helper or a genuinely standalone operation; say why in the PR when it is not obvious.
- Prefer one generic type over several near-identical specific ones when only a type parameter differs (`Database::listen::<T>()` returning `Listener<T>`, not one listener per payload).
- Make limits an operator may need to tune (concurrency, caps, request sizes, session and timeout lengths) configuration with a documented default. Internal tuning such as retry backoff or buffer sizes may stay a documented constant.
- Keep SQLx inside the `database` crate. Other crates go through `Database` and its repositories; a repository method takes a transaction only when the caller must compose several steps in it.
- Make items private by default and widen visibility only when another module needs them. Derive a trait only when something uses it. Implement `Debug`, `Clone`, and `PartialEq` on public types when sound, except on credentials (see Secrets and Observability).
- Put `#[must_use]` on builders, guards, and `Result`-like values that are a bug to drop.
- Before writing boilerplate (conversions, error derives, newtype wrappers), check whether a maintained, widely used crate such as `derive_more` provides it, and prefer the crate. Write it yourself only if the crate is unmaintained, pulls heavy transitive dependencies, or the need is trivial; say why in the PR description. Declare dependencies in `[workspace.dependencies]` with the minimum features. `cargo deny` (advisories, licenses, duplicates) gates CI.
- Do not write `macro_rules!` unless you can state the reason (for example preventing drift between parallel definitions) and include a doc example. Derive-style macros that generate companion types from one annotated definition are acceptable; look for an existing crate first. A custom proc-macro needs its own crate and an ADR explaining why no existing crate fits.
- Follow the relevant items of the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/). Do not make breaking changes to public APIs without proposing them first.

## Code Organization

- Group code into feature or domain modules. Split a module only when it has more than one responsibility or becomes hard to navigate.
- Use `module.rs` plus `module/child.rs`; never `mod.rs`.
- Order a file as constants, error types, other types, traits, implementations, then free functions, keeping related items adjacent. Put a type's `impl` blocks right under its definition.
- Keep a module's tests in its own `#[cfg(test)] mod tests`, not in a separate `tests/` directory, unless they exercise only the crate's public interface.
- Do not add wrapper modules, re-exports, or indirection without a caller that needs them.
- Put every `use` at the top of its module scope, including inside `mod tests`. Never write imports inside functions, blocks, or `impl` bodies.
- Import types and refer to them by name. Use an alias or a partial path only to resolve a name clash.
- For functions, import the parent module and call through it (`use std::fs;` then `fs::write(..)`). Do not import the function itself or write its full path, except to resolve a name clash.

## Naming

- Choose descriptive names that do not repeat their context (`Session::expires_at`, not `Session::session_expires_at`). Use the conventional single letters `T`, `U`, `E`, `K`, `V` for generic parameters.
- Name free functions as action plus object (`delete_expired_sessions`, not `delete_expired`). Methods may be shorter because the receiver supplies the object.
- Name a variable after the value it holds, not the step that produced it or a vague state: `result`, `photo`, `deleted_ids` rather than `served`, `validated`, `exhausted`, `done`. A reader must know what it contains without reading its initializer.
- Name fields and types after their role in this codebase, not after their mechanism or a near-synonym: `cancellation_token` for a `CancellationToken`, `workspaces` for a per-workspace map.
- Shadow a variable when you transform it without changing what it means. Use distinct names when both forms are still needed.
- Follow Rust naming conventions and use the domain terms already used in the codebase.

## Ownership and Borrowing

- Make data flow explicit through ownership and borrowing. Do not clone, wrap in `Arc`, add interior mutability, or allocate unless the borrow checker or an API requires it.

## Async and Concurrency

- Never block in async code. Run CPU-bound or blocking work through `spawn_blocking` or a bounded pool.
- Run every multi-step state change (claim, status update, publication) in one database transaction so a dropped future cannot leave partial state.
- Every spawned task needs an owner: join it, or cancel it through a shared shutdown signal. Never fire and forget.
- Keep in-memory state to data that is cheap to lose (rate limiters, caches, live subscribers). Anything that must survive a restart or stay consistent lives in the database.
- Use `std::sync::Mutex` for short critical sections and never hold a guard across `.await`; restructure with a channel or an owning task instead of using `tokio::sync::Mutex`.
- Bound every cross-task channel. When a consumer is slow, lag or drop it rather than buffering without limit.

## Readability and Errors

- Prefer readable code over clever code. Introduce intermediate variables and guard clauses to avoid deep nesting.
- Use `Option`/`Result` and iterator combinators for value transformations. Use `for`/`while` for side effects, async work, or when it is clearer.
- Always put a blank line:
  - before and after every control-flow block (`if`, `match`, `for`, `while`, `loop`);
  - after a group of `let` declarations, before the code that uses them;
  - before `return`, unless it is the only statement in its scope.
- Propagate recoverable errors with `Result` and `?`. Add context at abstraction boundaries, and treat a violated invariant as a bug (panic or assertion), not as a recoverable error.
- Define typed error enums per crate or domain boundary so callers can match on variants. Use `thiserror`. Make `Display` describe only the type's own layer and expose the cause through `source()`, so a chain is never printed twice. Report error chains (for example with `eyre`) only in a binary's `main`, never in a library.
- Wrap infrastructure causes as `source`; do not expose third-party error types (such as a database driver's) in public variants. Use `#[non_exhaustive]` only on types you intentionally publish.
- Only the `api` crate maps errors to HTTP status codes; domain crates never know about HTTP. Never send internal causes to clients; return the shared `ErrorResponse` shape.
- Respect the configured Clippy lints. `unwrap` and `expect` are allowed in tests. In production code they need a narrow lint allowance and a comment naming the invariant that makes them safe. Prefer `expect` when its message adds context.

## Secrets and Observability

- A type that holds a credential or bearer secret (passwords, tokens, share links that grant access to private data) must not derive `Debug`. Omit it, or implement a redacting `Debug` with a doc note.
- Never put secrets in logs or spans. When using `#[instrument]`, skip sensitive arguments. Give every request or job span an identifier.

## Comments and Docs

- Comments explain why (invariants, tradeoffs, ordering that matters, security-sensitive choices), not what the code does.
- Document a public item when its contract is not obvious from its signature: errors, invariants, and security or concurrency guarantees. Add a short runnable doc example to such items; `cargo test --workspace` runs them.
- Give a module whose behavior spans several functions or files a short `//!` overview of the flow and where each guarantee is enforced.
- Write documentation while implementing, not afterward; it is part of "done" before requesting review.
- Put documentation comments before all other attributes on an item.
- Keep documentation short and current. Remove comments that restate the code.
- Document the item's own contract. Do not restate another layer's implementation (migration names, SQL, trigger details) or reference issues, PRs or ticket ids; those age and belong in the PR or the design.

## Testing and Verification

- Use unit tests for isolated behavior and integration tests for interactions between components. Add a regression test for every bug fix.
- Keep tests deterministic and reuse existing test infrastructure.

Commands:

- Format: `cargo +nightly fmt --all` (nightly is required for the import-grouping settings in `rustfmt.toml`)
- Lint: `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Run the commands above for every Rust change.
