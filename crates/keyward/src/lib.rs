//! Keyward reference **Client**, exposed as a library so other front-ends (the CLI
//! `keyward` binary, the Tauri desktop app) can drive the same client core.
//!
//! The `client` module is the entry point: [`client::run`] dials a Node, authenticates
//! it, enforces policy, injects the credential locally, and relays the provider stream —
//! emitting structured [`client::ClientEvent`]s for any UI. The `session` module holds the
//! node-side protocol primitives (auth, pairing, a single-pair serve loop); `node` is the
//! deployable multi-tenant Node server (`keyward node`).

/// Re-exported wire/policy types, so embedders (e.g. the desktop app) can build a
/// [`keyward_proto::Policy`] without a separate dependency.
pub use keyward_proto;

pub mod client;
pub mod demo;
pub mod identity;
#[cfg(feature = "node")]
pub mod node;
pub mod pricing;
pub mod provider;
#[cfg(feature = "seal")]
pub mod seal;
pub mod secret;
pub mod session;
#[cfg(feature = "shim")]
pub mod shim;
pub mod transport;
pub mod wire;

#[cfg(test)]
mod e2e_tests;
