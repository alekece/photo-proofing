# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends libvips-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,id=pp-api-target \
    cargo build --release -p pp-api && cp target/release/pp-api /pp-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends curl libvips42 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home pp \
    && install -d -o pp -g pp /var/lib/pp/photos
COPY --from=build /pp-api /usr/local/bin/pp-api
# A fresh named volume copies this directory's ownership, so `pp` can write to it.
USER pp
ENV PP_PHOTO_DIR=/var/lib/pp/photos
ENV PP_HOST=0.0.0.0 PP_PORT=8080
EXPOSE 8080
HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${PP_PORT}/api/v1/health" || exit 1
ENTRYPOINT ["pp-api"]
