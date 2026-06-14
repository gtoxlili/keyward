//! Keyward v0 wire types and policy engine.
//!
//! This crate is the transport-agnostic core: the JSON envelope (§2), the message
//! bodies (§3–§7), the policy object (§6), and the policy enforcement order. It pulls
//! in no async runtime, no HTTP client, no crypto — so it compiles to every target
//! (CLI, `wasm32` for Workers/browser, Lambda) unchanged.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Protocol major version. Carried as the string `"0"` for this draft (§2, §11).
pub const KW: &str = "0";

/// A peer descriptor: who is on each end (§3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Stable identity of the peer (e.g. `node_…`). Optional on `client`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Provider-reported token usage (§5).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

/// A root-signed delegation of an operational key (§3, SSH-CA pattern). The root
/// signs `op_pubkey ‖ not_after`; the Client verifies that against the pinned
/// root, so operational keys can rotate without the Owner re-pairing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpCert {
    /// Operational public key, hex Ed25519.
    pub pubkey: String,
    /// Expiry, Unix seconds. The Client refuses an expired operational key.
    pub not_after: i64,
    /// The root's signature over the canonical cert bytes, hex.
    pub root_sig: String,
}

/// The full wire frame: envelope (§2) flattened over a typed body (§3–§7).
///
/// Response frames echo the originating intent's `mid`; `Body::WorkChunk.seq`
/// disambiguates ordering and drives gap detection / resume (§5, §7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub kw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    pub mid: String,
    #[serde(flatten)]
    pub body: Body,
}

impl Frame {
    pub fn new(sid: Option<String>, mid: impl Into<String>, body: Body) -> Self {
        Frame {
            kw: KW.to_string(),
            sid,
            mid: mid.into(),
            body,
        }
    }
}

/// Message bodies, internally tagged by `type` (§2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    /// Client → Node: pairing handshake (§3).
    Hello {
        pairing_token: String,
        client: Peer,
        providers: Vec<String>,
        policy_digest: String,
        /// Client's long-term identity pubkey, hex Ed25519. Lets the Node
        /// authenticate the Client (§9) — e.g. allow-list registered users.
        #[serde(skip_serializing_if = "Option::is_none")]
        pubkey: Option<String>,
        /// Client's signature over the `pairing_token`, proving possession of the
        /// `pubkey` identity (hex). Required when the Node authenticates
        /// Clients.
        #[serde(skip_serializing_if = "Option::is_none")]
        sig: Option<String>,
        /// Optional routing token the Client advertises to a multi-tenant **node**
        /// (§10): the node maps `route_token → this connection`, so a shared/public
        /// Node can route a request — which carries the token in its bearer
        /// header — to the right Client. Absent ⇒ the node derives a token from
        /// `pubkey`. Ignored by a single-tenant Node.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route_token: Option<String>,
    },
    /// Node → Client: session opened (§3).
    ///
    /// Resolves the §9 open question with the SSH-CA pattern: the Client pins the
    /// long-term `root_pubkey` on first contact (TOFU); each connection presents a
    /// short-lived operational key (`op`) delegated by the root, which signs the
    /// freshly-assigned `sid`. A stolen pairing token alone is useless (binding needs
    /// a key chaining to the pinned root), and the Node can rotate operational
    /// keys / autoscale across reconnects without forcing the Owner to re-pair.
    Paired {
        node: Peer,
        /// Long-term root identity, hex Ed25519. Pinned by the Client (TOFU).
        root_pubkey: String,
        /// Operational key for this connection + its root-signed delegation.
        op: OpCert,
        /// Operational key's detached signature over the assigned `sid`, hex.
        sig: String,
    },
    /// Node → Client: perform one provider call (§4).
    Work {
        provider: String,
        /// Provider-native request body, MINUS any credential (§4).
        request: Value,
    },
    /// Client → Node: intent passed policy, provider call started (§5).
    WorkAccepted {},
    /// Client → Node: one streamed chunk (§5).
    WorkChunk {
        /// Monotonic per-intent sequence, from 0. Drives gap detection and resume.
        seq: u64,
        /// Provider-native chunk, relayed verbatim.
        delta: Value,
    },
    /// Client → Node: terminal success (§5).
    WorkDone {
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        usage: Usage,
    },
    /// Client → Node: terminal failure (§5, §8).
    WorkError {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_status: Option<u16>,
    },
    /// Node → Client: resume a dropped intent from `last_seq` (§7).
    /// `last_seq = -1` means "from the beginning".
    Resume { intent_mid: String, last_seq: i64 },
    /// Node → Client: deliberately cancel an in-flight intent (§7).
    /// Distinct from a dropped channel: a drop suspends, a `cancel` aborts.
    Cancel { intent_mid: String },
    /// Either side: orderly teardown (§7).
    Close { reason: String },
    /// Envelope-level fault (§8). Never closes the channel by itself.
    Error { code: String, message: String },
    /// Opaque end-to-end ciphertext for the trustless-node case (§9/§10): the node
    /// relays it by `mid` without decrypting. Requester→Client it carries the sealed
    /// work (`hex(ephemeral_pubkey) ‖ sealed`); Client→Requester, the sealed response
    /// chunks/terminal. The plaintext shape is the seal layer's business, not the wire's.
    Sealed { blob: String },
}

// ---------------------------------------------------------------------------
// Policy (§6)
// ---------------------------------------------------------------------------

/// Owner-defined limits, enforced at the Client, not changeable by the
/// Node (§6). All fields optional; absence = unrestricted for that
/// dimension (but implementations SHOULD default-deny on budget).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub providers: Option<Vec<String>>,
    /// Model allowlist; entries MAY use a trailing `*` glob.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<Rate>,
    /// RFC3339 instant (UTC `Z`). Compared lexicographically in v0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub limit_usd: f64,
    pub window: String,
    #[serde(default)]
    pub spent_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpm: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tpm: Option<u32>,
}

/// A policy rejection, carrying the matching §8 error code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denied {
    Provider,
    Model,
    Node,
    Expired,
    Rate,
    Budget,
}

impl Denied {
    /// The `work_error.code` for this denial (§8).
    pub fn code(&self) -> &'static str {
        match self {
            Denied::Provider => "policy_provider",
            Denied::Model => "policy_model",
            Denied::Node => "policy_node",
            Denied::Expired => "policy_expired",
            Denied::Rate => "policy_rate",
            Denied::Budget => "policy_budget",
        }
    }
}

/// Live counters the Client threads into each check (rate/budget are stateful,
/// tracked by the Client — not by the Node, and not trusted from the wire).
#[derive(Debug, Clone, Copy, Default)]
pub struct Live<'a> {
    /// Requests already made in the current minute (for `rate.rpm`).
    pub rpm_used: u32,
    /// USD spent so far in the current budget window (for `budget.limit_usd`).
    pub spent_usd: f64,
    /// Current UTC instant as RFC3339, for `expires_at`.
    pub now_rfc3339: &'a str,
}

impl Policy {
    /// Enforce the policy in the §6 order:
    /// provider → model → node → expiry → rate → budget.
    /// Returns the first failing dimension, or `Ok(())`.
    pub fn check(&self, provider: &str, model: &str, node: &str, live: Live) -> Result<(), Denied> {
        if let Some(ps) = &self.providers
            && !ps.iter().any(|p| p == provider)
        {
            return Err(Denied::Provider);
        }
        if let Some(ms) = &self.models
            && !ms.iter().any(|m| glob_match(m, model))
        {
            return Err(Denied::Model);
        }
        if let Some(os) = &self.nodes
            && !os.iter().any(|o| o == node)
        {
            return Err(Denied::Node);
        }
        if let Some(exp) = &self.expires_at
            && !live.now_rfc3339.is_empty()
            && live.now_rfc3339 >= exp.as_str()
        {
            return Err(Denied::Expired);
        }
        if let Some(rate) = &self.rate
            && let Some(rpm) = rate.rpm
            && live.rpm_used >= rpm
        {
            return Err(Denied::Rate);
        }
        if let Some(b) = &self.budget {
            // Spend is tracked live by the Client; the policy only carries the cap.
            if live.spent_usd >= b.limit_usd {
                return Err(Denied::Budget);
            }
        }
        Ok(())
    }
}

/// Trailing-`*` glob match for model allowlists (§6). `"a-*"` matches `"a-foo"`;
/// otherwise an exact match.
pub fn glob_match(pattern: &str, value: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => value.starts_with(prefix),
        None => pattern == value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob() {
        assert!(glob_match("claude-3-5-sonnet-*", "claude-3-5-sonnet-20241022"));
        assert!(glob_match("gpt-4o", "gpt-4o"));
        assert!(!glob_match("gpt-4o", "gpt-4o-mini"));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn enforcement_order_provider_first() {
        let p = Policy {
            providers: Some(vec!["openai".into()]),
            models: Some(vec!["gpt-4o".into()]),
            ..Default::default()
        };
        // Wrong provider AND wrong model -> provider checked first.
        assert_eq!(
            p.check("anthropic", "claude", "acme", Live::default()),
            Err(Denied::Provider)
        );
    }

    #[test]
    fn budget_exhausted() {
        let p = Policy {
            budget: Some(Budget {
                limit_usd: 20.0,
                window: "month".into(),
                spent_usd: 0.0,
            }),
            ..Default::default()
        };
        let live = Live {
            spent_usd: 20.0,
            ..Default::default()
        };
        assert_eq!(p.check("openai", "gpt-4o", "acme", live), Err(Denied::Budget));
    }

    #[test]
    fn roundtrip_work_frame() {
        let f = Frame::new(
            Some("kw_sess_1".into()),
            "01J",
            Body::WorkChunk {
                seq: 7,
                delta: serde_json::json!({"x": 1}),
            },
        );
        let s = serde_json::to_string(&f).unwrap();
        let back: Frame = serde_json::from_str(&s).unwrap();
        match back.body {
            Body::WorkChunk { seq, .. } => assert_eq!(seq, 7),
            _ => panic!("wrong body"),
        }
        assert!(s.contains("\"type\":\"work_chunk\""));
        assert!(s.contains("\"kw\":\"0\""));
    }

    #[test]
    fn roundtrip_paired_with_op_cert() {
        let f = Frame::new(
            Some("kw_sess_1".into()),
            "01J",
            Body::Paired {
                node: Peer {
                    name: "acme".into(),
                    version: None,
                    id: Some("node_1".into()),
                },
                root_pubkey: "9d8f".into(),
                op: OpCert {
                    pubkey: "1a2b".into(),
                    not_after: 1779999999,
                    root_sig: "cafe".into(),
                },
                sig: "beef".into(),
            },
        );
        let s = serde_json::to_string(&f).unwrap();
        let back: Frame = serde_json::from_str(&s).unwrap();
        match back.body {
            Body::Paired {
                root_pubkey, op, sig, ..
            } => {
                assert_eq!(root_pubkey, "9d8f");
                assert_eq!(op.not_after, 1779999999);
                assert_eq!(op.pubkey, "1a2b");
                assert_eq!(sig, "beef");
            }
            _ => panic!("wrong body"),
        }
        assert!(s.contains("\"type\":\"paired\""));
    }

    #[test]
    fn roundtrip_resume_and_cancel() {
        for (body, tag) in [
            (
                Body::Resume {
                    intent_mid: "m1".into(),
                    last_seq: 41,
                },
                "resume",
            ),
            (
                Body::Cancel {
                    intent_mid: "m1".into(),
                },
                "cancel",
            ),
        ] {
            let f = Frame::new(Some("s".into()), "x", body);
            let s = serde_json::to_string(&f).unwrap();
            assert!(s.contains(&format!("\"type\":\"{tag}\"")));
            // round-trips back to the same variant
            let back: Frame = serde_json::from_str(&s).unwrap();
            let s2 = serde_json::to_string(&back).unwrap();
            assert_eq!(s, s2);
        }
    }

    #[test]
    fn unknown_fields_are_ignored() {
        // Forward-compat (§2/§11): receivers must ignore unknown fields.
        let json = r#"{"kw":"0","type":"work_accepted","mid":"m","sid":"s","future_field":42}"#;
        let f: Frame = serde_json::from_str(json).unwrap();
        assert!(matches!(f.body, Body::WorkAccepted {}));
    }
}
