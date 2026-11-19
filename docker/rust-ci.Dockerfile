ARG UBUNTU_BASE=ubuntu:24.04
FROM ${UBUNTU_BASE}
ARG UBUNTU_BASE

LABEL org.opencontainers.image.base.name="${UBUNTU_BASE}" \
      org.opencontainers.image.source="https://github.com/alekece/photo-proofing"

RUN apt-get -o Acquire::Retries=2 -o Acquire::http::Timeout=30 update \
    && apt-get -o Acquire::Retries=2 -o Acquire::http::Timeout=30 upgrade -y \
    && apt-get -o Acquire::Retries=2 -o Acquire::http::Timeout=30 install -y --no-install-recommends \
        build-essential ca-certificates curl git libvips-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
