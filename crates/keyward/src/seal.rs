//! End-to-end seal layer for the trustless-broker case (§9/§10).
//!
//! The requester (shim) seals each request to the Executor's **identity** key; the broker
//! relays only ciphertext; the Executor decrypts, runs, and seals the reply back. The
//! broker never sees the prompt — only the routing token (in the envelope) and opaque blobs.
//!
//! Profile: X25519 ECDH between a fresh ephemeral (requester) and the Executor's static
//! key — both **converted from the §3 Ed25519 identities** (the libsodium
//! ed25519→curve25519 maps) — feeding ChaCha20-Poly1305 (`crypto_box`). The request
//! carries the ephemeral pubkey; the derived box encrypts the response stream too. This is
//! the *non-interactive* MVP of the §9 inner layer: it needs no multi-message handshake
//! relayed through the broker, but gives no per-message forward secrecy (compromising the
//! Executor's long-term key later exposes recorded traffic). Interactive Noise XX, with FS,
//! is the documented upgrade.

use anyhow::{Result, anyhow};
use crypto_box::aead::{Aead, AeadCore, OsRng};
use crypto_box::{ChaChaBox, Nonce, PublicKey, SecretKey};
use ed25519_dalek::{SigningKey, VerifyingKey};
use sha2::{Digest, Sha512};

use crate::wire::{hex, unhex};

/// Executor X25519 secret from its Ed25519 signing key
/// (libsodium `crypto_sign_ed25519_sk_to_curve25519`): clamp SHA-512(seed)[..32].
pub fn x25519_secret(ed: &SigningKey) -> SecretKey {
    let h = Sha512::digest(ed.to_bytes());
    let mut s = [0u8; 32];
    s.copy_from_slice(&h[..32]);
    s[0] &= 248;
    s[31] &= 127;
    s[31] |= 64;
    SecretKey::from(s)
}

/// Executor X25519 public from its Ed25519 identity public (Montgomery u-coordinate) —
/// what the shim derives from the Executor's known identity pubkey.
pub fn x25519_public(ed: &VerifyingKey) -> PublicKey {
    PublicKey::from(ed.to_montgomery().to_bytes())
}

/// A symmetric authenticated channel from one X25519 ECDH — same box on both sides.
pub struct Sealed {
    cbox: ChaChaBox,
}

impl Sealed {
    /// Requester side: a fresh ephemeral keypair → the channel + the ephemeral pubkey to
    /// hand the Executor (hex).
    pub fn initiator(executor_pub: &PublicKey) -> (Self, String) {
        let eph = SecretKey::generate(&mut OsRng);
        let epk = hex(&eph.public_key().to_bytes());
        (
            Self {
                cbox: ChaChaBox::new(executor_pub, &eph),
            },
            epk,
        )
    }

    /// Executor side: reconstruct the channel from its static secret + the ephemeral pubkey.
    pub fn responder(executor_secret: &SecretKey, eph_pub_hex: &str) -> Result<Self> {
        let b = unhex(eph_pub_hex)
            .filter(|b| b.len() == 32)
            .ok_or_else(|| anyhow!("bad ephemeral pubkey"))?;
        let mut epk = [0u8; 32];
        epk.copy_from_slice(&b);
        Ok(Self {
            cbox: ChaChaBox::new(&PublicKey::from(epk), executor_secret),
        })
    }

    /// Seal `plaintext` → `nonce(24) ‖ ciphertext`, hex.
    pub fn seal(&self, plaintext: &[u8]) -> Result<String> {
        let nonce = ChaChaBox::generate_nonce(&mut OsRng);
        let ct = self
            .cbox
            .encrypt(&nonce, plaintext)
            .map_err(|_| anyhow!("seal failed"))?;
        let mut out = nonce.as_slice().to_vec();
        out.extend_from_slice(&ct);
        Ok(hex(&out))
    }

    /// Open a `nonce(24) ‖ ciphertext` hex blob.
    pub fn open(&self, blob: &str) -> Result<Vec<u8>> {
        let raw = unhex(blob).ok_or_else(|| anyhow!("bad sealed hex"))?;
        if raw.len() < 24 {
            return Err(anyhow!("sealed blob too short"));
        }
        let nonce = Nonce::from_slice(&raw[..24]);
        self.cbox
            .decrypt(nonce, &raw[24..])
            .map_err(|_| anyhow!("open failed (wrong key or tampered)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ed_to_x25519_public_agrees() {
        // The pubkey the shim derives from the identity must match the one the
        // Executor's secret derives — else ECDH wouldn't agree.
        let ed = SigningKey::generate(&mut rand_core::OsRng);
        assert_eq!(
            x25519_secret(&ed).public_key().to_bytes(),
            x25519_public(&ed.verifying_key()).to_bytes()
        );
    }

    #[test]
    fn seal_round_trips_and_wrong_key_fails() {
        let exec = SigningKey::generate(&mut rand_core::OsRng);
        let exec_pub = x25519_public(&exec.verifying_key());

        // shim seals a request to the executor's identity pubkey
        let (shim, epk) = Sealed::initiator(&exec_pub);
        let req = shim.seal(br#"{"prompt":"secret"}"#).unwrap();

        // executor reconstructs from its secret + the ephemeral pubkey, decrypts
        let exec_chan = Sealed::responder(&x25519_secret(&exec), &epk).unwrap();
        assert_eq!(exec_chan.open(&req).unwrap(), br#"{"prompt":"secret"}"#);

        // executor seals a reply; the shim opens it
        let reply = exec_chan.seal(b"pong").unwrap();
        assert_eq!(shim.open(&reply).unwrap(), b"pong");

        // a different key (a curious broker) cannot open the request
        let other = SigningKey::generate(&mut rand_core::OsRng);
        let attacker = Sealed::responder(&x25519_secret(&other), &epk).unwrap();
        assert!(attacker.open(&req).is_err());
    }
}
