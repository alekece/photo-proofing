# syntax=docker/dockerfile:1
# Toolchain image for the Rust CI jobs: Rust, clippy and the native libraries the workspace links.
FROM rust:1-trixie
RUN apt-get update && apt-get install -y --no-install-recommends libvips-dev pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add clippy
