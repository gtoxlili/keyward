//! Node identity: the root → operational-key chain (§3/§9).
//!
//! The Client pins a long-term **root** key on first contact. Each connection
//! presents a short-lived **operational** key carrying a root-signed delegation
//! (`OpCert`) and signs the assigned `sid` with it. The Client accepts any
//! operational key whose cert chains to the pinned root — so the Node can
//! rotate keys / autoscale across reconnects without the Owner re-pairing, while a
//! stolen pairing token alone still can't bind (it can't forge a root signature).

use anyhow::{Result, anyhow};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use keyward_proto::OpCert;

use crate::wire::{hex, unhex};

/// Current Unix time in seconds (for cert issuance/expiry checks).
pub fn now_unix() -> i64 {
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

/// Node side: have the root delegate a fresh operational key until `not_after`.
pub fn issue_op_cert(root: &SigningKey, op_pub: &VerifyingKey, not_after: i64) -> OpCert {
    let sig = root.sign(&cert_msg(&op_pub.to_bytes(), not_after));
    OpCert {
        pubkey: hex(&op_pub.to_bytes()),
        not_after,
        root_sig: hex(&sig.to_bytes()),
    }
}

/// Client side: verify the cert chains to `root` and isn't expired; return the op key.
pub fn verify_op_cert(root: &VerifyingKey, cert: &OpCert, now: i64) -> Result<VerifyingKey> {
    let op = parse_pubkey(&cert.pubkey)?;
    if cert.not_after < now {
        return Err(anyhow!("operational key expired"));
    }
    let sig = parse_sig(&cert.root_sig)?;
    root.verify(&cert_msg(&op.to_bytes(), cert.not_after), &sig)
        .map_err(|_| anyhow!("operational key is not signed by the pinned root"))?;
    Ok(op)
}

/// Sign a detached message, returning a hex signature.
pub fn sign_detached(key: &SigningKey, msg: &[u8]) -> String {
    crate::wire::hex(&key.sign(msg).to_bytes())
}

/// Verify a detached hex signature by `pubkey` over `msg`.
pub fn verify_detached(pubkey: &VerifyingKey, msg: &[u8], sig_hex: &str) -> Result<()> {
    pubkey
        .verify(msg, &parse_sig(sig_hex)?)
        .map_err(|_| anyhow!("signature invalid"))
}

/// Verify an operational key's signature over the assigned `sid`.
pub fn verify_sid_sig(op: &VerifyingKey, sid: &str, sig_hex: &str) -> Result<()> {
    verify_detached(op, sid.as_bytes(), sig_hex)
}

pub fn parse_pubkey(hex_str: &str) -> Result<VerifyingKey> {
    let b: [u8; 32] = unhex(hex_str)
        .ok_or_else(|| anyhow!("bad pubkey hex"))?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("pubkey must be 32 bytes"))?;
    VerifyingKey::from_bytes(&b).map_err(|e| anyhow!("bad pubkey: {e}"))
}

/// Load the Client's persistent identity (keychain / env), or generate one and
/// persist it. The public half is what an Node allow-lists.
pub fn load_or_create_identity() -> SigningKey {
    if let Some(seed) = crate::secret::load_identity_seed()
        && let Some(bytes) = unhex(&seed)
        && let Ok(arr) = <[u8; 32]>::try_from(bytes.as_slice())
    {
        return SigningKey::from_bytes(&arr);
    }
    let key = SigningKey::generate(&mut rand_core::OsRng);
    let _ = crate::secret::store_identity_seed(&crate::wire::hex(&key.to_bytes()));
    key
}

fn parse_sig(hex_str: &str) -> Result<Signature> {
    let b: [u8; 64] = unhex(hex_str)
        .ok_or_else(|| anyhow!("bad signature hex"))?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("signature must be 64 bytes"))?;
    Ok(Signature::from_bytes(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;

    fn keypair() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn valid_chain_verifies() {
        let root = keypair();
        let op = keypair();
        let cert = issue_op_cert(&root, &op.verifying_key(), 10_000);
        let got = verify_op_cert(&root.verifying_key(), &cert, 9_000).unwrap();
        assert_eq!(got.to_bytes(), op.verifying_key().to_bytes());
    }

    #[test]
    fn wrong_root_rejected() {
        let root = keypair();
        let attacker = keypair();
        let op = keypair();
        let cert = issue_op_cert(&root, &op.verifying_key(), 10_000);
        assert!(verify_op_cert(&attacker.verifying_key(), &cert, 9_000).is_err());
    }

    #[test]
    fn expired_rejected() {
        let root = keypair();
        let op = keypair();
        let cert = issue_op_cert(&root, &op.verifying_key(), 1_000);
        assert!(verify_op_cert(&root.verifying_key(), &cert, 2_000).is_err());
    }

    #[test]
    fn sid_signature_roundtrips() {
        let op = keypair();
        let sig = hex(&op.sign(b"kw_sess_abc").to_bytes());
        assert!(verify_sid_sig(&op.verifying_key(), "kw_sess_abc", &sig).is_ok());
        assert!(verify_sid_sig(&op.verifying_key(), "kw_sess_xyz", &sig).is_err());
    }
}
