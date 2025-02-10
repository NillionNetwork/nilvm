FROM public.ecr.aws/x8g8t2h7/rust-builder-gha:0.9.7

ENV SELF_VERSION="0.9.7"
ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
      musl-tools \
      gcc-aarch64-linux-gnu \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /install

COPY scripts/install/setup-toolchain.sh /install/scripts/install/setup-toolchain.sh
RUN rustup target add x86_64-unknown-linux-musl
RUN scripts/install/setup-toolchain.sh x86_64-unknown-linux-musl /opt

RUN rustup target add aarch64-unknown-linux-musl
RUN scripts/install/setup-toolchain.sh aarch64-unknown-linux-musl /opt

WORKDIR /root
