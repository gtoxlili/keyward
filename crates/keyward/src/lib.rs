//! Keyward reference Executor, exposed as a library so other front-ends (the CLI
//! `keyward` binary, the Tauri desktop app) can drive the same executor core.
//!
//! The `executor` module is the entry point: [`executor::run`] dials an Orchestrator,
//! authenticates it, enforces policy, injects the credential locally, and relays the
//! provider stream — emitting structured [`executor::ExecutorEvent`]s for any UI.

pub mod demo;
pub mod executor;
pub mod identity;
pub mod orchestrator;
pub mod pricing;
pub mod provider;
#[cfg(feature = "proxy")]
pub mod proxy;
pub mod secret;
pub mod transport;
pub mod wire;

#[cfg(test)]
mod e2e_tests;
