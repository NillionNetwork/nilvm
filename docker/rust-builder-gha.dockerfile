FROM ubuntu:oracular-20241120

ENV SELF_VERSION="0.9.7"
ARG docker_ver=26.1.3-0ubuntu1
ARG DEBIAN_FRONTEND="noninteractive"

SHELL ["/bin/bash", "-o", "pipefail", "-xe", "-c"]

RUN set -e; \
    apt update; \
    apt upgrade -y; \
    # Install the bare minimum dependencies to be able to install everything at once
    apt install -y --no-install-recommends \
      # curl to be able to set node up
      curl \
      # certificates to be able to hit the node packages endpoint below
      ca-certificates; \
    # set up the nodejs package sources
    curl -fsSL https://deb.nodesource.com/setup_21.x | bash -; \
    # now install everything we need
    apt install -y --no-install-recommends \
        awscli \
        build-essential \
        cmake \
        curl \
        docker-buildx \
        docker.io="${docker_ver}" \
        git \
        jq \
        m4 \
        mold \
        nodejs \
        pandoc \
        pkg-config \
        shellcheck \
        ssh \
        sudo \
        wget \
        zlib1g-dev; \
    apt clean; \
    rm -rf /var/lib/apt/lists/*

WORKDIR /install

COPY rust-toolchain.toml /install/rust-toolchain.toml
COPY scripts/install/rust.sh /install/scripts/install/rust.sh
RUN scripts/install/rust.sh

COPY scripts/install/ci-cargo-config.toml /root/.cargo/config.toml

COPY scripts/install/tooling.sh /install/scripts/install/tooling.sh
RUN scripts/install/tooling.sh

COPY scripts/install/uv.sh /install/scripts/install/uv.sh
RUN scripts/install/uv.sh

ENV PATH="/opt/toolchains/linux/aarch64-unknown-linux-musl/bin:/opt/toolchains/linux/x86_64-unknown-linux-musl/bin:${PATH}"

ENV PATH          /root/.local/bin:/root/.cargo/bin:/root/.rustup/bin:$PATH
ENV CARGO_HOME    /root/.cargo
ENV RUSTUP_HOME   /root/.rustup

WORKDIR /root
