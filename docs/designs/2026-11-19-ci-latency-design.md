# Reduce pull request CI wait time

Tracking issue [#398](https://github.com/alekece/photo-proofing/issues/398).

Preserve required status names, existing test coverage, path-based skips, and successful main CI as the deployment gate.

## Pin the Rust CI image

Run Clippy and Rust tests in the same GHCR image pinned by digest. Build it on Ubuntu 24.04 with libvips development files and pkg-config installed. Preserve toolchain setup and Cargo caching. Rust tests keep PostgreSQL 17 and use the service hostname from the job container.

A trusted workflow on main rebuilds weekly and on demand. Resolve the current Ubuntu base to a digest and rebuild with fresh package indexes and no cached package installation. Compare the base digest and installed package versions with the current image. When either changes, publish a candidate and open or update one PR that writes the single digest pin in `.github/rust-ci-image` read by both consumers. The PR runs normal CI and requires human merge approval. Retain the previous digest for rollback.

Keep package-write access in the publisher. Establish how the refresh workflow can create PRs and run checks before enabling the schedule. Verify that the pinned image is pullable from regular PR jobs with read-only permissions.

Per-job APT timeouts cap mirror stalls. The selected image approach removes per-run package downloads and adds scheduled image maintenance.

## Benchmark Rust tests

Run the API and database suites on different runners. Assign every other workspace package exactly once and preserve doctests and PostgreSQL-backed tests. Preserve the required Rust tests status with an aggregate that reports failure if either group fails and respects the existing path filter. Compare elapsed time, runner time, and test counts on the same revision with comparable caches. Retain the split if the elapsed improvement justifies runner overhead.

## Benchmark web test environments

Keep jsdom as the default and opt clearly DOM-free tests into Node.js. Keep component and accessibility tests on jsdom. Run the full web suite and compare counts and hosted timings. Retain the changes if they improve elapsed time. Evaluate two Vitest shards only if the first change yields little improvement.
