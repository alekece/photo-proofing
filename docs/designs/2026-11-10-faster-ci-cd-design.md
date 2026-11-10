# Faster CI and CD

Issue: [#331](https://github.com/alekece/photo-proofing/issues/331)

## Problem

The image build sets CI's wall-clock time (5.5–6.5 min) and CD rebuilds the same images from scratch (5–6.5 min). Both build the three images one after another with no cache.

## Decisions

- **CD owns image building and publishing.** CI stays read-only (`contents: read`); only CD has `packages: write`.
- **CI builds images only on PRs that can break them:** changes to `docker/**`, `Cargo.lock` or `web/pnpm-lock.yaml`. Code changes are already built and tested by the Rust and web jobs. It no longer builds on pushes to `main`, since CD does.
- **Images build in parallel** (matrix) in both workflows.
- **Shared layer cache** (`type=gha`, one scope per image). CD writes it on `main`; PRs read it (a PR cannot write a cache that `main` reads).
- **CD builds with `--push` directly** instead of `--load` then `docker push`. The "skip if `sha-…` exists" check stays per image.
- **Staging promotion unchanged:** verify all three images, advance the `staging` tag, then move the `:staging` image tags. It stays a single job with the existing concurrency group, so ordering safeguards are untouched.

## Out of scope

- Prebuilt base images and a `libvips` layer (follow-up in #331).
- Retagging unchanged images instead of rebuilding; the layer cache already makes them fast.
