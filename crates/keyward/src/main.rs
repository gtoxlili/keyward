//! Keyward CLI — the reference **Client** (holds the key, dials out) and **Node**
//! (the rendezvous the unaware SaaS points at).
//!
//!   keyward demo          run the self-contained end-to-end demo (default)
//!   keyward client        dial out to a Node, hold the key, serve provider calls
//!   keyward node          run a Node: many Clients dial in, routed by API-key token

use keyward::{client, demo, identity, secret, wire};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "demo".into());
    match cmd.as_str() {
        "demo" => demo::run().await,
        "resume-demo" => demo::run_resume().await,
        "client" => client::run_cli().await,
        "set-key" => set_key_cli(),
        "delete-key" => delete_key_cli(),
        "identity" => identity_cli(),
        "node" => run_node().await,
        "shim" => run_shim().await,
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

async fn run_node() -> anyhow::Result<()> {
    #[cfg(feature = "node")]
    {
        keyward::node::run_cli().await
    }
    #[cfg(not(feature = "node"))]
    {
        anyhow::bail!("`keyward node` needs a build with --features node")
    }
}

async fn run_shim() -> anyhow::Result<()> {
    #[cfg(feature = "shim")]
    {
        keyward::shim::run_cli().await
    }
    #[cfg(not(feature = "shim"))]
    {
        anyhow::bail!("`keyward shim` needs a build with --features shim")
    }
}

/// Print this Client's identity pubkey + fingerprint, to register with a Node that
/// allow-lists Clients.
fn identity_cli() -> anyhow::Result<()> {
    let vk = identity::load_or_create_identity().verifying_key().to_bytes();
    println!("client identity");
    println!("  fingerprint: {}", wire::fingerprint(&vk));
    println!("  pubkey:      {}", wire::hex(&vk));
    println!("\nGive the pubkey to a Node to be allow-listed (e.g. KEYWARD_AUTHORIZED_CLIENTS).");
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
        "keyward — non-custodial BYOK: a Node (rendezvous) + a Client (holds the key)\n\n\
         USAGE:\n  \
           keyward demo          self-contained end-to-end demo (no key, no network)\n  \
           keyward resume-demo   drop-the-channel resume + cancel demo (§7)\n  \
           keyward client        dial out to a Node, hold the key, serve provider calls\n                        \
             env: KEYWARD_NODE_URL, KEYWARD_PAIRING_TOKEN, KEYWARD_ROUTE_TOKEN\n  \
           keyward set-key <p>   store provider <p>'s key in the OS keychain (key via stdin)\n  \
           keyward delete-key <p> remove provider <p>'s key from the OS keychain\n  \
           keyward identity      print this Client's identity pubkey (to be allow-listed)\n  \
           keyward node          run a Node: Clients dial in, requests routed by the API-key token (--features node)\n                        \
             env: KEYWARD_LISTEN, KEYWARD_HTTP_LISTEN, KEYWARD_PAIRING_TOKEN, KEYWARD_SINGLE_TENANT\n  \
           keyward shim          requester-side shim: seals to the Client, relays via a blind Node (--features shim)\n                        \
             env: KEYWARD_SHIM_LISTEN, KEYWARD_NODE_URL, KEYWARD_ROUTE_TOKEN, KEYWARD_CLIENT_PUBKEY"
    );
}
