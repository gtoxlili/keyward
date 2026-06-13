#!/usr/bin/env bash
# Regenerate the vendored model-price snapshot from LiteLLM's registry.
# Run when prices drift, then commit the result. See SPEC §6 — provider-reported
# usage is the billing source of truth; this is the per-model rate table.
set -euo pipefail

url="https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json"
dst="$(cd "$(dirname "$0")/.." && pwd)/crates/keyward/data/model_prices.json"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

echo "fetching $url"
curl -fsSL "$url" -o "$tmp"

python3 - "$tmp" "$dst" <<'PY'
import json, sys
src, dst = sys.argv[1], sys.argv[2]
data = json.load(open(src))
out = {}
for model, spec in data.items():
    if model == "sample_spec" or not isinstance(spec, dict):
        continue
    i = spec.get("input_cost_per_token")
    o = spec.get("output_cost_per_token")
    if isinstance(i, (int, float)) and isinstance(o, (int, float)):
        out[model] = [i, o]
with open(dst, "w") as f:
    f.write(json.dumps(out, separators=(",", ":"), sort_keys=True) + "\n")
print(f"wrote {len(out)} models -> {dst}")
PY
