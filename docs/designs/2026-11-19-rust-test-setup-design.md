# Reduce repeated Rust test setup

Tracking issue [#401](https://github.com/alekece/photo-proofing/issues/401).

Reduce the time PhotoProofing spends setting up Rust tests in CI while preserving the observable behavior they cover.

## Approach

Start with the repeated work identified in the audit. This gives a smaller change with a clear before-and-after comparison. Replacing every API signup fixture has broader reach, so consider a small session-fixture pilot separately if the first pass leaves API setup as a bottleneck. Keep the existing Cargo runner and one database per SQLx test.

## Scope

- Set the two login limiter tests to a threshold of two. Keep real password verification and assert that a second peer has an independent limit. Retain the assertion that production configuration defaults to ten.
- Use one app with distinct peer addresses in the middleware status matrix. Preserve every status and its expected effect on the failure bucket.
- Opt the standalone PostgreSQL error and malformed-notification tests out of migrations. Keep migrations for tests of application schema and constraints.
- Merge export snapshot creation with snapshot immutability, photo retry with configured attempts, and the three gallery-open notification cases. Keep all assertions and separate tests for independent authorization, concurrency, and migration contracts.
- Replace the oversized export request case with an effective 1,000/1,001 boundary check. Preserve the separate duplicate-ID and foreign-ID behavior.
- Extend database tests.rs only for fixture setup shared by multiple modules. Keep scenario-specific setup and assertions alongside their tests. API HTTP setup stays in api/src/tests.rs.

Production configuration, security behavior, workflow check names, and the pinned CI image stay the same. The root-only blind spot in the storage permissions test is separate because it requires runner changes.

## Verification

Run the repository's Rust format, lint, and test checks. Confirm the resized auth tests still exercise both counting and non-counting cases, and that the export boundary fails if the limit check is removed. Run PostgreSQL-backed tests with PostgreSQL 17. Compare hosted Rust test timings on equivalent revisions with comparable cache states. Record removed setup work separately from measured wall-clock savings.
