//! Budget pricing. Per SPEC §6 the answer to "where does pricing come from" is
//! LiteLLM's registry: a vendored snapshot (`data/model_prices.json`, regenerated
//! by `scripts/refresh-prices.sh`) is embedded as the pinned fallback, and budget
//! is metered against provider-reported `usage`. A scheduled refresh from the live
//! URL is left to the deployment (the spec calls it implementation-defined).

use std::collections::HashMap;
use std::sync::LazyLock;

use keyward_proto::Usage;

/// Vendored snapshot: model -> (input_cost_per_token, output_cost_per_token) USD.
static PRICES: LazyLock<HashMap<String, (f64, f64)>> = LazyLock::new(|| {
    let raw = include_str!("../data/model_prices.json");
    serde_json::from_str(raw).unwrap_or_default()
});

/// USD cost of a call from provider-reported usage and per-model pricing.
pub fn cost_usd(model: &str, usage: &Usage) -> f64 {
    let (in_cost, out_cost) = price_per_token(model);
    usage.input_tokens as f64 * in_cost + usage.output_tokens as f64 * out_cost
}

/// (input, output) USD per token. Tries, in order: exact `model`, exact bare name
/// (relays / OpenRouter namespace models as `provider/name`), longest-prefix
/// `model`, longest-prefix bare name; then a conservative default so budget still
/// bounds spend on an unknown model.
fn price_per_token(model: &str) -> (f64, f64) {
    let bare = model.split_once('/').map(|(_, b)| b);
    if let Some(&hit) = PRICES.get(model) {
        return hit;
    }
    if let Some(b) = bare {
        if let Some(&hit) = PRICES.get(b) {
            return hit;
        }
    }
    if let Some(p) = prefix_match(model) {
        return p;
    }
    if let Some(p) = bare.and_then(prefix_match) {
        return p;
    }
    (1.0 / 1e6, 3.0 / 1e6)
}

/// The longest registry key that is a prefix of `model` (handles dated/variant ids).
fn prefix_match(model: &str) -> Option<(f64, f64)> {
    let mut best: Option<(&str, (f64, f64))> = None;
    for (k, &v) in PRICES.iter() {
        if model.starts_with(k.as_str()) && best.is_none_or(|(bk, _)| k.len() > bk.len()) {
            best = Some((k.as_str(), v));
        }
    }
    best.map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads_many_models() {
        assert!(
            PRICES.len() > 500,
            "vendored price registry should load (got {})",
            PRICES.len()
        );
    }

    #[test]
    fn known_model_has_a_price() {
        let u = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
        };
        assert!(cost_usd("gpt-4o", &u) > 0.0, "gpt-4o should be priced");
    }

    #[test]
    fn unknown_model_uses_conservative_default() {
        let u = Usage {
            input_tokens: 1_000_000,
            output_tokens: 0,
        };
        let c = cost_usd("totally-made-up-model-xyz", &u);
        assert!(
            (c - 1.0).abs() < 1e-9,
            "unknown -> $1 per 1M input tokens, got {c}"
        );
    }

    #[test]
    fn dated_variant_matches_base_by_prefix() {
        let u = Usage {
            input_tokens: 1_000_000,
            output_tokens: 0,
        };
        assert_eq!(cost_usd("gpt-4o", &u), cost_usd("gpt-4o-2099-01-01", &u));
    }

    #[test]
    fn namespaced_model_matches_bare_name() {
        // Relays / OpenRouter send "openai/gpt-4o-mini"; it must price like the bare name,
        // not fall through to the unknown-model default. (Found via live verification.)
        let u = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
        };
        let bare = cost_usd("gpt-4o-mini", &u);
        assert!(bare > 0.0);
        assert_eq!(cost_usd("openai/gpt-4o-mini", &u), bare);
    }
}
