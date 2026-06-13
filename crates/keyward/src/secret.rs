//! Provider-credential storage for the local Executor.
//!
//! Best practice (2026): keep the key in the OS keychain, not a dotenv/env var.
//! The Executor resolves a per-provider credential from the keychain first, then
//! falls back to an env var; it is always wrapped in a `SecretString` (redacted
//! Debug, zeroized on drop) and is never read from argv. Keys are resolved per
//! provider, so one Executor can front several providers (one credential each).

use anyhow::{anyhow, Result};
use secrecy::{ExposeSecret, SecretString};

const SERVICE: &str = "keyward";

/// Where the Executor gets the credential for a provider call.
pub enum KeySource {
    /// A single fixed credential (the demo / tests).
    Fixed(SecretString),
    /// Resolve per provider from the OS keychain, then env (the real CLI).
    Keychain,
}

impl KeySource {
    pub fn resolve(&self, provider: &str) -> SecretString {
        match self {
            KeySource::Fixed(s) => SecretString::from(s.expose_secret().to_string()),
            KeySource::Keychain => load_key(credential_provider(provider)),
        }
    }
}

/// Map an API-surface provider to the account whose credential it uses. The
/// Responses API and Chat Completions share one OpenAI key.
fn credential_provider(provider: &str) -> &str {
    match provider {
        "openai-responses" => "openai",
        p => p,
    }
}

/// Keychain first, then `KEYWARD_PROVIDER_KEY` / the provider's conventional env
/// var. Returns an empty secret when nothing is set (the mock path needs no key).
pub fn load_key(provider: &str) -> SecretString {
    if let Some(k) = from_keychain(provider) {
        return k;
    }
    let env = std::env::var("KEYWARD_PROVIDER_KEY")
        .or_else(|_| std::env::var(provider_env_var(provider)))
        .unwrap_or_default();
    SecretString::from(env)
}

fn provider_env_var(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "ANTHROPIC_API_KEY",
        _ => "OPENAI_API_KEY",
    }
}

fn from_keychain(provider: &str) -> Option<SecretString> {
    let entry = keyring::Entry::new(SERVICE, provider).ok()?;
    entry.get_password().ok().map(SecretString::from)
}

/// Store a credential in the OS keychain (`keyward set-key <provider>`).
pub fn store_key(provider: &str, secret: &str) -> Result<()> {
    keyring::Entry::new(SERVICE, provider)
        .and_then(|e| e.set_password(secret))
        .map_err(|e| anyhow!("keychain write failed: {e}"))
}

/// Remove a stored credential (`keyward delete-key <provider>`).
pub fn delete_key(provider: &str) -> Result<()> {
    keyring::Entry::new(SERVICE, provider)
        .and_then(|e| e.delete_credential())
        .map_err(|e| anyhow!("keychain delete failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_key_resolves_without_touching_the_keychain() {
        let s = KeySource::Fixed(SecretString::from("sk-abc".to_string()));
        assert_eq!(s.resolve("openai").expose_secret(), "sk-abc");
        assert_eq!(s.resolve("anthropic").expose_secret(), "sk-abc");
    }

    #[test]
    fn provider_env_var_mapping() {
        assert_eq!(provider_env_var("anthropic"), "ANTHROPIC_API_KEY");
        assert_eq!(provider_env_var("openai"), "OPENAI_API_KEY");
        assert_eq!(provider_env_var("groq"), "OPENAI_API_KEY");
    }

    #[test]
    fn responses_shares_the_openai_credential() {
        assert_eq!(credential_provider("openai-responses"), "openai");
        assert_eq!(credential_provider("anthropic"), "anthropic");
    }
}
