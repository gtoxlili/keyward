# keyward-grpc

The **gRPC transport adapter** for Keyward.

The protocol is transport-agnostic (spec [§1](../../docs/spec.md)): it needs exactly one
reliable, ordered, bidirectional, message-oriented channel. This crate provides that
over a gRPC bidirectional stream and exposes the **same `Frame` channels** the
WebSocket adapter does — so the Client and the Node SDK run identical logic
on top, whichever transport carries it.

```protobuf
message Frame { string json = 1; }          // one canonical Keyward JSON message
service Keyward {
  rpc Open(stream Frame) returns (stream Frame);   // one bidi stream = one session
}
```

- The **Client** dials OUT — it is the gRPC **client** ([`dial`]) — so the
  no-inbound-ports invariant still holds.
- The **Node** listens — gRPC **server** ([`accept_one`]).
- gRPC is the pipe; the JSON envelope from the spec is unchanged (`Frame { json }`
  wraps it). A fully-typed protobuf profile may come later.

Both entry points return `(mpsc::Sender<Frame>, mpsc::Receiver<Frame>)`: send on the
first, receive on the second. The bin's `transport` module and the SDK's
`serve_one_grpc` are thin wrappers over them.

## Build

`protoc` is required (tonic compiles `proto/keyward.proto` at build time). This is the
only crate in the workspace with that requirement, so it is kept out of the default
build set — a plain `cargo build` stays pure-Rust; gRPC builds when you opt in:

```sh
cargo build -p keyward --features grpc        # client (gRPC client)
cargo build -p keyward-sdk --features grpc     # node SDK (gRPC server)
# macOS:  brew install protobuf      Debian/Ubuntu:  apt-get install protobuf-compiler
```

[`dial`]: https://docs.rs/keyward-grpc
[`accept_one`]: https://docs.rs/keyward-grpc
