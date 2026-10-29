# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,id=pp-api-target \
    cargo build --release -p pp-api && cp target/release/pp-api /pp-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home pp
COPY --from=build /pp-api /usr/local/bin/pp-api
USER pp
ENV PP_HOST=0.0.0.0 PP_PORT=8080
EXPOSE 8080
HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${PP_PORT}/api/v1/health" || exit 1
ENTRYPOINT ["pp-api"]
