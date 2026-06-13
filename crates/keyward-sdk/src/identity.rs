//! Orchestrator-side crypto: the root → operational-key chain and Executor
//! authentication. The byte formats here MUST match the Executor's verifier.

use anyhow::{Result, anyhow, bail};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use keyward_proto::{Body, Frame, OpCert, Peer};
use rand_core::OsRng;

use crate::Config;
use crate::wire::{hex, unhex};

pub fn new_mid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Canonical bytes the root signs to delegate an operational key: `pubkey ‖ not_after`.
fn cert_msg(op_pubkey: &[u8; 32], not_after: i64) -> Vec<u8> {
    let mut m = Vec::with_capacity(40);
    m.extend_from_slice(op_pubkey);
    m.extend_from_slice(&not_after.to_le_bytes());
    m
}

/// Build the `paired` frame: a fresh operational key delegated by the root, signing
/// the assigned `sid` (the SSH-CA chain). Returns `(sid, frame)`.
pub fn build_paired(cfg: &Config) -> (String, Frame) {
    let sid = format!("kw_sess_{}", &new_mid()[..8]);
    let op = SigningKey::generate(&mut OsRng);
    let not_after = now_unix() + 3600;
    let root_sig = cfg
        .root
        .sign(&cert_msg(&op.verifying_key().to_bytes(), not_after));
    let cert = OpCert {
        pubkey: hex(&op.verifying_key().to_bytes()),
        not_after,
        root_sig: hex(&root_sig.to_bytes()),
    };
    let sig = op.sign(sid.as_bytes());
    let frame = Frame::new(
        Some(sid.clone()),
        new_mid(),
        Body::Paired {
            orchestrator: Peer {
                name: cfg.name.clone(),
                version: None,
                id: Some(cfg.id.clone()),
            },
            root_pubkey: hex(&cfg.root.verifying_key().to_bytes()),
            op: cert,
            sig: hex(&sig.to_bytes()),
        },
    );
    (sid, frame)
}

/// The orchestrator's root identity fingerprint (show this to the Owner for OOB
/// confirmation).
pub fn root_fingerprint(cfg: &Config) -> String {
    crate::wire::fingerprint(&cfg.root.verifying_key().to_bytes())
}

/// Authenticate the Executor from its `hello`: pairing token, possession proof
/// (signature over the token), and the optional allow-list.
pub fn authenticate_executor(hello: &Body, cfg: &Config) -> Result<()> {
    let Body::Hello {
        pairing_token,
        pubkey,
        sig,
        ..
    } = hello
    else {
        bail!("expected hello");
    };
    if pairing_token != &cfg.pairing_token {
        bail!("pairing token rejected");
    }
    match (pubkey, sig) {
        (Some(pk), Some(sig)) => {
            let vk = parse_pubkey(pk)?;
            let sig = parse_sig(sig)?;
            vk.verify(cfg.pairing_token.as_bytes(), &sig)
                .map_err(|_| anyhow!("executor identity signature invalid"))?;
        }
        _ => {
            if cfg.authorized_executors.is_some() {
                bail!("executor identity required but not provided");
            }
        }
    }
    if let Some(allow) = &cfg.authorized_executors {
        let pk = pubkey.as_deref().unwrap_or("");
        if !allow.iter().any(|a| a == pk) {
            bail!("executor not authorized");
        }
    }
    Ok(())
}

fn parse_pubkey(hex_str: &str) -> Result<VerifyingKey> {
    let b: [u8; 32] = unhex(hex_str)
        .ok_or_else(|| anyhow!("bad pubkey hex"))?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("pubkey must be 32 bytes"))?;
    VerifyingKey::from_bytes(&b).map_err(|e| anyhow!("bad pubkey: {e}"))
}

fn parse_sig(hex_str: &str) -> Result<ed25519_dalek::Signature> {
    let b: [u8; 64] = unhex(hex_str)
        .ok_or_else(|| anyhow!("bad signature hex"))?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("signature must be 64 bytes"))?;
    Ok(ed25519_dalek::Signature::from_bytes(&b))
}
