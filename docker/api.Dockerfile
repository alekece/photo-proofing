# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,id=eg-api-target \
    cargo build --release -p eg-api && cp target/release/eg-api /eg-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home eg
COPY --from=build /eg-api /usr/local/bin/eg-api
USER eg
ENV EG_HOST=0.0.0.0 EG_PORT=8080
EXPOSE 8080
HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${EG_PORT}/api/v1/health" || exit 1
ENTRYPOINT ["eg-api"]
