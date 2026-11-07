# Release build on tags implementation plan

Design: [release build on tags](../designs/2026-11-07-release-build-on-tags-design.md). Issue: [#210](https://github.com/alekece/photo-proofing/issues/210).

1. Remove the `rust-build` job from `.github/workflows/ci.yml`.
2. Add `.github/workflows/release.yml` (`Release`; `v[0-9]*` tags and `workflow_dispatch`; job `rust-release-build`, name `Rust release build`; `contents: read`; `needs:` contract comment).
3. Add one line to `README.md` next to the CI reference.
4. Validate with `actionlint`; confirm `staging` does not match the tag pattern.
5. Independent review, refactor pass, delete this plan, open PR stating the manual branch-protection step.
