# Typed libvips options

Issue: #142. Builds on `docs/designs/2026-11-04-proof-derivatives-design.md`.

## Goal

Require libvips 8.15 or newer everywhere and replace the option strings in `crates/image-worker/src/render.rs` with the typed `*_with_opts` calls of the `libvips` crate where they work, so option names and values are checked at compile time. Output does not change: auto-rotated, sRGB, no metadata, never upscaled, with the same bounded decoding.

## Approaches

1. **Full typed (chosen).** Raise libvips and type every option: the JPEG decode (`shrink`, `fail-on`) and the JPEG save (`Q`, `strip`).
2. **Typed output only.** Leaves the decode strings, which are the easiest to mistype. Rejected: the issue asks for typed options.
3. **Image bump only.** Does not deliver the outcome. Rejected.

## Behaviour

- `render.rs` uses typed options for encoding. Decoding keeps its option string: the crate's typed `jpegload_buffer_with_opts` passes the output-only `flags` argument as an input and crashes libvips 8.15 (found in CI on Ubuntu 24.04). The module comment explains the libvips 8.15 requirement.
- Option values keep their meaning: `shrink` from the existing scale table, truncated files still fail, quality 75, metadata stripped.
- Thumbnail and preview bytes need not be identical to before, but the existing derivative and colour-profile tests pass unchanged.
- If a typed option cannot express a string option exactly (for example the ICC profile field name, or `strip` with `keep`), the closest typed equivalent is used and the difference is noted in the PR.

## Build and tooling

- `docker/worker.Dockerfile` and `docker/api.Dockerfile` move from bookworm to trixie (libvips 8.16) for both build and runtime stages. The API links libvips too, so it changes with the worker.
- CI jobs that install `libvips-dev` run on a runner image that ships 8.15 or newer (`ubuntu-24.04` or later); a job that cannot reach 8.15 that way is a blocker to raise, not worked around silently.
- README build prerequisites state the minimum (8.15) and list distributions that ship it: Debian trixie, Ubuntu 24.04 and newer, Fedora, Arch and Homebrew. Older distributions such as Ubuntu 22.04 (8.12) are listed as unsupported. No build-from-source or dev container guidance.

## Non-goals

- No change to derivative sizes, quality, colour handling or the job queue.
- No libvips build-from-source path or dev container.
- No change to web images (they do not use libvips).

## Verification

- `crates/` format, lint and test commands from `crates/AGENTS.md`, run against libvips 8.15 or newer.
- Worker and API images build.

## Left to the builder

The exact `libvips` crate option types, and whether the version minimum is also enforced at runtime (not required).
