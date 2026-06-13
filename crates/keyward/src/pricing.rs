//! Budget pricing. The research answer to the §6 open question is: vendor
//! LiteLLM's `model_prices_and_context_window.json`, refresh on a schedule, and
//! keep a pinned fallback copy. This v0 skeleton ships a tiny embedded table so
//! the metering loop is exercised end-to-end; swap in the vendored data next.

use keyward_proto::Usage;

/// USD cost of a call, from provider-reported usage and per-model pricing.
pub fn cost_usd(model: &str, usage: &Usage) -> f64 {
    let (in_per_mtok, out_per_mtok) = price_per_mtok(model);
    (usage.input_tokens as f64 / 1e6) * in_per_mtok + (usage.output_tokens as f64 / 1e6) * out_per_mtok
}

/// (input, output) USD per 1M tokens. Stand-in values — replace with LiteLLM data.
fn price_per_mtok(model: &str) -> (f64, f64) {
    match model {
        m if m.starts_with("gpt-4o-mini") => (0.15, 0.60),
        m if m.starts_with("gpt-4o") => (2.50, 10.00),
        m if m.starts_with("claude-3-5-sonnet") => (3.00, 15.00),
        _ => (1.00, 3.00), // unknown model -> conservative default
    }
}
