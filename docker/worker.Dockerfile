# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,id=eg-worker-target \
    cargo build --release -p eg-image-worker && cp target/release/eg-image-worker /eg-image-worker

FROM debian:bookworm-slim
RUN useradd --system --no-create-home eg
COPY --from=build /eg-image-worker /usr/local/bin/eg-image-worker
USER eg
# The worker has no HTTP listener: it is healthy while its process is PID 1,
# so a crashed or replaced entrypoint is reported as unhealthy.
HEALTHCHECK --interval=5s --timeout=3s --start-period=3s --retries=3 \
    CMD grep -qx eg-image-worker /proc/1/comm || exit 1
ENTRYPOINT ["eg-image-worker"]
