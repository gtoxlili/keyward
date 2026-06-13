//! Keyward reference Executor + a mock Orchestrator for the end-to-end demo.
//!
//!   keyward demo          run the self-contained end-to-end demo (default)
//!   keyward executor      dial out to an Orchestrator (env-configured)
//!   keyward orchestrator  serve a single-prompt Orchestrator for manual testing

use keyward::{demo, executor, identity, orchestrator, secret, wire};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "demo".into());
    match cmd.as_str() {
        "demo" => demo::run().await,
        "resume-demo" => demo::run_resume().await,
        "executor" => executor::run_cli().await,
        "orchestrator" => orchestrator::run_cli().await,
        "set-key" => set_key_cli(),
        "delete-key" => delete_key_cli(),
        "identity" => identity_cli(),
        "proxy" => run_proxy().await,
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => {
            eprintln!("unknown subcommand: {other}\n");
            print_usage();
            std::process::exit(2);
        }
    }
}

async fn run_proxy() -> anyhow::Result<()> {
    #[cfg(feature = "proxy")]
    {
        keyward::proxy::run_cli().await
    }
    #[cfg(not(feature = "proxy"))]
    {
        anyhow::bail!("`keyward proxy` needs a build with --features proxy")
    }
}

/// Print this Executor's identity pubkey + fingerprint, to register with an
/// Orchestrator / SaaS that allow-lists Executors.
fn identity_cli() -> anyhow::Result<()> {
    let vk = identity::load_or_create_identity().verifying_key().to_bytes();
    println!("executor identity");
    println!("  fingerprint: {}", wire::fingerprint(&vk));
    println!("  pubkey:      {}", wire::hex(&vk));
    println!("\nGive the pubkey to an orchestrator to be allow-listed (e.g. KEYWARD_AUTHORIZED_EXECUTORS).");
    Ok(())
}

/// Store a provider key in the OS keychain. The secret is read from a hidden
/// terminal prompt (no echo) when interactive, or from stdin when piped — never
/// from argv, where it would be visible in `ps` / shell history.
fn set_key_cli() -> anyhow::Result<()> {
    use std::io::{BufRead, IsTerminal};
    let provider = std::env::args().nth(2).ok_or_else(|| {
        anyhow::anyhow!("usage: keyward set-key <provider>   (paste the key, or pipe it on stdin)")
    })?;
    let secret = if std::io::stdin().is_terminal() {
        rpassword::prompt_password(format!("paste the {provider} key (hidden): "))?
    } else {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        line.trim_end_matches(['\n', '\r']).to_string()
    };
    if secret.is_empty() {
        anyhow::bail!("empty key — nothing stored");
    }
    secret::store_key(&provider, &secret)?;
    println!("stored '{provider}' key in the OS keychain (service 'keyward')");
    Ok(())
}

fn delete_key_cli() -> anyhow::Result<()> {
    let provider = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("usage: keyward delete-key <provider>"))?;
    secret::delete_key(&provider)?;
    println!("deleted '{provider}' key from the OS keychain");
    Ok(())
}

fn print_usage() {
    eprintln!(
        "keyward — non-custodial BYOK executor (v0 skeleton)\n\n\
         USAGE:\n  \
           keyward demo          self-contained end-to-end demo (no key, no network)\n  \
           keyward resume-demo   drop-the-channel resume + cancel demo (§7)\n  \
           keyward executor      dial out to an Orchestrator (keys from OS keychain, then env)\n                        \
             env: KEYWARD_ORCH_URL, KEYWARD_PAIRING_TOKEN\n  \
           keyward set-key <p>   store provider <p>'s key in the OS keychain (key via stdin)\n  \
           keyward delete-key <p> remove provider <p>'s key from the OS keychain\n  \
           keyward identity      print this Executor's identity pubkey (to be allow-listed)\n  \
           keyward proxy         OpenAI-compatible HTTP proxy backed by a paired executor (--features proxy)\n                        \
             env: KEYWARD_LISTEN, KEYWARD_PROXY_LISTEN, KEYWARD_PAIRING_TOKEN\n  \
           keyward orchestrator  serve a single-prompt mock Orchestrator\n                        \
             env: KEYWARD_LISTEN, KEYWARD_PAIRING_TOKEN, KEYWARD_PROVIDER, KEYWARD_MODEL,\n                        \
                  KEYWARD_PROMPT, KEYWARD_AUTHORIZED_EXECUTORS"
    );
}
