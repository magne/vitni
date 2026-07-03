# CI build image for the genealogy workspace.
#
# Bakes everything ci.yml's `check` job would otherwise install on every run: the Dioxus
# desktop webview dev libraries, the pinned Rust stable toolchain with the components and
# the wasm32-wasip2 plugin target from rust-toolchain.toml, and cargo-nextest. Published to
# ghcr.io/magne/genealogy-ci. Postgres integration tests run in a separate native job, so
# this image needs no Docker client.
#
# Rust is pinned to match rust-toolchain.toml's resolved stable at image-build time. The
# toolchain file still governs at runtime; baking the same version avoids a re-download.
FROM rust:1.96.1-bookworm

# arm64-ready: buildx sets TARGETARCH per platform. Today only linux/amd64 is published
# (see ci-image.yml); the arch-aware steps below already handle arm64 when it is added.
ARG TARGETARCH

# Dioxus 0.7 desktop (genealogy-ui-dioxus, feature `desktop`) links a system webview
# (wry/tao). Union of the Dioxus 0.7 Ubuntu list and what ci.yml installed previously.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libwebkit2gtk-4.1-dev \
        libgtk-3-dev \
        libxdo-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        libssl-dev \
        lld \
        pkg-config \
        build-essential \
        curl \
        wget \
        file \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Match rust-toolchain.toml: components rustfmt/clippy/rust-analyzer + the wasm plugin target.
RUN rustup component add rustfmt clippy rust-analyzer \
    && rustup target add wasm32-wasip2

# cargo-nextest, pinned. get.nexte.st serves x86_64 as `linux`, aarch64 as `linux-arm`.
RUN case "$TARGETARCH" in \
        amd64) NX_PLATFORM=linux ;; \
        arm64) NX_PLATFORM=linux-arm ;; \
        *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && curl -LsSf "https://get.nexte.st/0.9.138/${NX_PLATFORM}" | tar zxf - -C "${CARGO_HOME}/bin"

ENV CARGO_TERM_COLOR=always
