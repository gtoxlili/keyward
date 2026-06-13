# syntax=docker/dockerfile:1
#
# Keyward server image — one binary, several server roles:
#
#   # OpenAI-compatible gateway (default): your app points OPENAI_BASE_URL at :8088,
#   # the Owner's executor dials into :8787.
#   docker run -p 8088:8088 -p 8787:8787 keyward
#
#   # Reference orchestrator, or an always-on executor on the Owner's box:
#   docker run keyward orchestrator
#   docker run -e KEYWARD_ORCH_URL=grpc://orch.example.com:443 \
#              -e KEYWARD_PAIRING_TOKEN=pt_... -e OPENAI_API_KEY=sk-... keyward executor
#
# Built with the proxy, both real provider dialects, and the gRPC transport. See
# docs/ for the integration and user guides.

# ---- build stage -----------------------------------------------------------------
FROM rust:1-slim-bookworm AS builder
# protoc is needed for the gRPC adapter's tonic codegen (keyward-grpc).
RUN apt-get update \
    && apt-get install -y --no-install-recommends protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
# Cache the cargo registry and target dir across builds; copy the binary out before
# the cache mount is unmounted.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p keyward --features proxy,openai,anthropic,grpc \
    && cp target/release/keyward /usr/local/bin/keyward

# ---- runtime stage ---------------------------------------------------------------
FROM debian:bookworm-slim
# CA roots for outbound TLS to providers. (reqwest uses rustls with bundled webpki
# roots, so this is belt-and-suspenders for anything that consults the system store.)
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 -m -d /home/keyward keyward
COPY --from=builder /usr/local/bin/keyward /usr/local/bin/keyward

# The CLI defaults to 127.0.0.1; inside a container bind reachable interfaces instead.
ENV KEYWARD_LISTEN=0.0.0.0:8787 \
    KEYWARD_PROXY_LISTEN=0.0.0.0:8088
# 8088 = OpenAI-compatible HTTP front (your app); 8787 = WebSocket the executor dials in.
EXPOSE 8088 8787

USER keyward
WORKDIR /home/keyward
ENTRYPOINT ["keyward"]
CMD ["proxy"]
