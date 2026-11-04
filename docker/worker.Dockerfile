# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends libvips-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,id=pp-worker-target \
    cargo build --release -p pp-image-worker && cp target/release/pp-image-worker /pp-image-worker

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libvips42 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home pp \
    && install -d -o pp -g pp /var/lib/pp/photos
COPY --from=build /pp-image-worker /usr/local/bin/pp-image-worker
# A fresh named volume copies this directory's ownership, so `pp` can write to it.
USER pp
ENV PP_PHOTO_DIR=/var/lib/pp/photos
# The worker has no HTTP listener: it is healthy while its process is PID 1,
# so a crashed or replaced entrypoint is reported as unhealthy.
HEALTHCHECK --interval=5s --timeout=3s --start-period=3s --retries=3 \
    CMD grep -qx pp-image-worker /proc/1/comm || exit 1
ENTRYPOINT ["pp-image-worker"]
