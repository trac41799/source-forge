# Implementation detail — decision contract & backends

## Endpoint
`POST {base_url}/v1/systemone`
- Hosted: base `https://openrouter.ai/api` → `https://openrouter.ai/api/v1/systemone`
  (equivalent alt path: `https://openrouter.ai/api/alpha/decisions`).
  Auth: `Authorization: Bearer $OPENROUTER_API_KEY`. Model `jev-1.13` (maps to
  `typesafe/jev-1.13`) or `~typesafe/jev-latest`.
- Local: base = `decision.base_url` (e.g. `http://127.0.0.1:8009`).
- The existing `chat/completions` path is **not** used for decisions.

## Request
```json
{
  "model": "jev-1.13",
  "state": "<string | JSON object | array of text>",
  "questions": {
    "department": { "type": "choice", "instructions": "Which team should handle this?",
                    "criteria": { "billing": "...", "technical": "...", "other": "..." } },
    "urgency":    { "type": "score",  "instructions": "How urgent is this?",
                    "criteria": ["low", "medium", "high"] },
    "urgent":     { "type": "noul",   "instructions": "Does this message express urgency?" }
  }
}
```
All questions answered in **one parallel pass**. Question names never reach the model —
`instructions` carry the meaning (reference `state` fields in backticks). Include an
explicit `other`/`none` option so the model can decline. Caps: 255 choice options; 2–10
score levels.

## Wire response
```json
{
  "model": "typesafe/jev-1.13-20260917",
  "answers": {
    "department": { "type":"choice", "choice":"billing",
                    "probabilities": {"billing":0.8,"technical":0.15,"other":0.05}, "confidence":0.82 },
    "urgency":    { "type":"score", "score":1.15,
                    "legend": {"0":"low","1":"medium","2":"high"},
                    "probabilities": {"0":0.0,"1":0.85,"2":0.15}, "confidence":0.77 },
    "urgent":     { "type":"noul", "noul":0.87 }
  },
  "usage": { "input_tokens": 275, "output_tokens": 0, "cost": 0.00003 },
  "id": "gen-dec-…", "provider": "TypeSafe"
}
```

## Normalized (adapter output — what ACC code consumes)
```json
{
  "answers": {
    "department": { "kind":"choice", "value":"billing", "probabilities": {...}, "confidence":0.82 },
    "urgency":    { "kind":"score",  "value":1.15, "legend": {...}, "probabilities": {...}, "confidence":0.77 },
    "urgent":     { "kind":"noul",   "value":0.87, "confidence":0.87 }
  },
  "usage": { "input_tokens": 275, "cost": 0.00003 },
  "model": "typesafe/jev-1.13-20260917"
}
```

## Normalization rules
- `choice` → `value`; assert `value ∈ criteria`; probabilities **indexed by name**, never position.
- `score` → `value` = probability-weighted average of levels (ordinal 0-based); keep
  `legend` and raw `probabilities`. Jev already returns this weighted average in the wire
  `score` field, so the adapter **passes it through** (verified live 2026-09-26:
  probabilities `{0:0.62,1:0.38,2:0}` → `score 0.38`). Tier mapping is the **consumer's**
  job (e.g. score < 0.5 → low, < 1.5 → medium, else high) — never inferred from the number alone.
- `noul` → `value ∈ [0,1]`; **confidence is derived (`confidence = value`)** because the
  wire has no confidence for noul. Near 0.5 = uncertain → review band (R50).
- `probability` sums asserted ≈ 1.0 (±0.001) for choice/score.
- On HTTP error / malformed body → typed `DecisionError`; caller follows ADR 0002.

## Privacy (constitution §4, spec R5/R6)
- **Never store or log `state`** (tickets/documents/PTY contain customer data). Log only
  `id`, question names, probabilities, confidence, and the applied threshold.
- Hosted PTY egress requires the hosted backend to be the selected backend (R21).
- Key = `OPENROUTER_API_KEY` from env/vault; never in source, logs, or client bundles.

## `decision_usage` row (migration 016)
`id, backend_id, model, primitives, answers, confidence, policy_outcome, latency_ms,
input_tokens, cost, truncated, created_at` — no `state` column.

## Backends (all expose `/v1/systemone`; swappable by config)
| Backend | Access | Size / notes | Best for |
|---|---|---|---|
| TypeSafe **Jev** `typesafe/jev-1.13` | OpenRouter `/v1/systemone` | hosted, output free, 70–500 ms, 32k ctx | **default**, zero-ops |
| **Von** `wfzyx/von-1.0` | `pip install von-sdk` | ModernBERT ~395M, CPU/Metal/CUDA | compact local default |
| **Laya** | `pip install laya` | ModernBERT-large 322–421M, 100+ langs | multilingual (zh/vi) |
| **Kev** | local server (`/v1/systemone`) | Qwen3.5 0.8B/4B/9B + LoRA | closest local replica |
| **Rizzo Flow** | local server | Spark 1.7–4B, llama.cpp | easy serving |
| **NanoJev** | local server | Qwen3-0.6B, batchable | high-volume embedded |
| **SemIf** | CLI / lib | direct logits, Qwen3.5-4B | no-fine-tune experiments |
| ~~djev / OpenJev~~ | — | image/multimodal/thinking | **not applicable** (text only) |

> **Caveat — "swappable by config" means HTTP.** The adapter always speaks
> `POST {base_url}/v1/systemone` over HTTP. Local projects that ship as a *library*
> (e.g. `von-sdk`, `laya`) require a small serving shim (or their own server mode) to
> expose that HTTP contract before they can be selected as a backend.

## Fallback matrix (R51) — verified line refs
| Consumer | Fallback when backend unavailable |
|---|---|
| daemon router | prior LLM prompt + `parse_router_response` (`local-daemon/router.py:50`) |
| compounder category | LLM-emitted category (`knowledge.rs:782,808`) |
| KG typing | LLM-emitted type JSON (`kg_extraction.rs:28,46`) |
| route_task | hardcoded defaults vec (`routing.rs:93-111`) |
| outcome | keyword heuristic (`intelligence.rs:597`) |
| budget complexity | caller-supplied value / static table (`budget.rs:69-79`) |
| failure confidence | leave 0.0 |
| handoff verify | schema-only check (`handoff_parser.rs:26`) |
| deployment verify | deterministic checks only (`verification.rs:93`) |
| contradiction/merge | `jaccard_similarity` (`knowledge.rs:1008,1036`; shared-word `knowledge.rs:312`) |

## Privacy / egress (R6, R31)
`decision_usage` stores no `state` and no secret (R6); the API key never appears in a payload
or log. **Note (R31):** the semantic secrets check *must see* the wave's changes, so the collected
wave diff — including untracked/gitignored secret-named file bodies (bounded) — is sent to the
configured backend. That is by design (detecting a secret requires inspecting it); the backend is
the operator-selected endpoint, and this egress is the only place non-README content leaves the
machine.
