# Operator guide — decision layer (spec 001)

## Backends
The decision layer speaks one contract: `POST {base_url}/v1/systemone` with
`{model, state, questions}`. Select a backend in **Settings → Decision Layer** (M5).

- **Hosted (default):** base `https://openrouter.ai/api`, model `typesafe/jev-1.13`,
  auth `OPENROUTER_API_KEY`. Zero-ops.
- **Local (offline):** any server exposing `/v1/systemone` — Von, Laya, Kev, Rizzo Flow.
  Set `decision.backend = local` and `decision.base_url = http://127.0.0.1:<port>`.
  No key needed for localhost.

## Config (`decision_config` table, migration 016)
| Key | Default | Meaning |
|---|---|---|
| `backend` | `hosted` | `hosted` or `local` |
| `base_url` | `https://openrouter.ai/api` | endpoint base |
| `model` | `typesafe/jev-1.13` | model id |
| `accept_threshold` | `0.75` | confidence ≥ this → act |
| `review_threshold` | `0.40` | `[review, accept)` → review queue; below → fallback |
| `context_limit` | `32000` | max tokens of `state` (estimate = ceil(chars/4)) |
| `timeout_ms` | `5000` | per-request timeout |

`review_threshold` must be ≤ `accept_threshold` (enforced on write).

## Thresholds & review (ADR 0002)
Decisions below `accept_threshold` are not auto-applied. Consumers with a review path
(routing, handoff, contradiction) write a `decision_reviews` row; others fall back to
their prior behaviour. Tune the bands on a labeled sample per workload before rollout.

## Local backend quick start (example: Von)
```bash
pip install von-sdk          # then run its /v1/systemone server
# Settings → Decision Layer: backend=local, base_url=http://127.0.0.1:<port>
```

## Privacy
- `state` is **never** stored or logged; `decision_usage` holds only model, answers,
  confidence, policy outcome, latency, tokens, cost, truncated.
- The API key is read from env/vault and never written to source, logs, or client bundles.
- Hosted egress of PTY text happens only when the hosted backend is selected.

## Health
Settings shows `healthy | degraded | offline`. `health_probe` sends an empty state and a
single reachability `noul` — it carries no real data.

## Cost
Hosted Jev is ~$0.042 per million input tokens, output free. Decisions are batched into a
single parallel call per state.
