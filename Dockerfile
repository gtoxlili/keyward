# syntax=docker/dockerfile:1
#
# Keyward server image — one Rust binary, several server roles. Built with the node role,
# both provider dialects, and the gRPC transport.
#
#   docker build -t keyward .
#   docker run -p 8088:8088 -p 8787:8787 keyward            # OpenAI-compatible gateway (default)
#   docker run keyward demo                                  # self-contained end-to-end demo
#   docker run -e KEYWARD_NODE_URL=grpc://node.example.com:443 \
#              -e KEYWARD_PAIRING_TOKEN=pt_... -e OPENAI_API_KEY=sk-... keyward client
#
# cargo-chef caches the dependency build in its own layer — deps recompile only when
# Cargo.{toml,lock} change, not on every source edit. Base images use floating tags so
# each build picks up the latest:
#   - cargo-chef on slim-trixie: tracks the latest stable Rust, and matches the
#     distroless debian13 glibc generation (both trixie) so there's no link-time drift.
#   - distroless cc-debian13:nonroot: ~8 MB, glibc + ca-certs, no shell, runs non-root.
ARG RUST_IMAGE=lukemathwalker/cargo-chef:latest-rust-slim-trixie
ARG RUNTIME_IMAGE=gcr.io/distroless/cc-debian13:nonroot

# --- chef base: cargo-chef + protoc (the gRPC adapter's tonic codegen) -------------
FROM ${RUST_IMAGE} AS chef
WORKDIR /src
RUN apt-get update \
    && apt-get install -y --no-install-recommends protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
ENV CARGO_TERM_COLOR=always

# --- planner: derive the dependency recipe from the manifests alone ----------------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- builder: cook deps into a cached layer, then build the binary -----------------
FROM chef AS builder
ARG TARGETARCH
COPY --from=planner /src/recipe.json recipe.json
# Per-arch cache scopes so parallel multi-arch builds don't race in the registry.
RUN --mount=type=cache,id=cargo-registry-${TARGETARCH},target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git-${TARGETARCH},target=/usr/local/cargo/git \
    cargo chef cook --release -p keyward --features node,openai,anthropic,grpc --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,id=cargo-registry-${TARGETARCH},target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git-${TARGETARCH},target=/usr/local/cargo/git \
    cargo build --release -p keyward --features node,openai,anthropic,grpc \
    && cp target/release/keyward /tmp/keyward

# --- runner: distroless cc (glibc + ca-certs, no shell), non-root ------------------
FROM ${RUNTIME_IMAGE} AS runner
COPY --from=builder --chown=nonroot:nonroot /tmp/keyward /usr/local/bin/keyward
# The CLI defaults to 127.0.0.1; bind reachable interfaces inside the container.
ENV KEYWARD_LISTEN=0.0.0.0:8787 \
    KEYWARD_HTTP_LISTEN=0.0.0.0:8088
# 8088 = OpenAI-compatible HTTP front (your app); 8787 = WebSocket the client dials in.
EXPOSE 8088 8787
USER nonroot
ENTRYPOINT ["/usr/local/bin/keyward"]
CMD ["node"]
