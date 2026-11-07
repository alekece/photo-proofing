# Frontend build identity implementation plan

Goal: expose the build identity approved in
`docs/designs/2026-11-07-build-identity-design.md` and issue #227.
Use Vite's typed build constant, the production Docker build argument and the
existing deployment revision. Implement inline in this isolated worktree.

- [x] Add failing component tests for short SHA, full-SHA copying, explicit
  clipboard failure/manual copy, and Development with no copy action.
- [x] Embed `PP_BUILD_SHA` with Vite; absent means development, malformed values
  fail the build. Add a small Settings-only BuildInfo component.
- [x] Pass the revision to the production web image in CD. Document local builds
  and Settings behavior. Verify a synthetic-SHA build contains that exact value.
- [x] Run frontend verify; obtain one independent review and perform a scoped
  refactor pass. Remove this plan after corrections; create PR and squash-merge
  only after green CI, as explicitly authorized.

Evidence: all 516 tests / 71 files, lint/types/format and build passed.
A synthetic full SHA appears in both compiled Settings client and server
assets. A malformed SHA fails the build explicitly. Independent reviewer found
no material issues. Refactor pass found no additional abstraction warranted.
Remaining delivery gate: PR, plan cleanup and green CI before squash merge.
