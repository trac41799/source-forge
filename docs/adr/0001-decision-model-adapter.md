# ADR 0001 — One decision-model adapter, hosted Jev default, local open-weight option

**Status:** Proposed · **Date:** 2026-09-26 · **Spec:** 001-decision-layer

## Context
ACC needs typed, calibrated decisions (choice / score / noul) in ~11 places. A hosted
model (TypeSafe Jev, OpenRouter slug `typesafe/jev-1.13`) is zero-ops. But ACC is branded
local-first, and in September 2026 a field of open-weight decision models appeared (Von
≈395M ModernBERT, Laya 322–421M multilingual, Kev 0.8–9B, Rizzo Flow, NanoJev, SemIf).
Committing to a single vendor would contradict constitution §4 (local-first) and create
lock-in. The interface is shared because two concrete backends are planned from the
start — not because they already exist (constitution §2).

## Decision
Introduce **one** decision interface (`src-tauri/src/decision.rs`) with a configurable
backend. **Pinned contract** (verified against OpenRouter docs, 2026-09-26):
- `POST {base_url}/v1/systemone`; hosted base `https://openrouter.ai/api`
  (equivalent alt path `…/api/alpha/decisions`); local base = `decision.base_url`.
- Request: `{ "model": <id>, "state": <string|object|array>, "questions": { <name>: { type, instructions, criteria? } } }`.
- Response: `{ "model", "answers": { <name>: {type, choice?, score?, legend?, probabilities?, confidence?, noul?} }, "usage": {input_tokens, output_tokens, cost}, "id", "provider" }`.
- **`noul` has no wire `confidence`** — the adapter derives `confidence = noul`.
- Local backends (Von/Laya/Kev/Rizzo) expose the same path and shapes.

No per-vendor code paths; backend choice is configuration, not code.

## Consequences
- **Positive:** one module to test; hosted→local swap is a config change; local-first
  promise upheld; no lock-in; reuses existing OpenRouter auth.
- **Negative:** minor response-shape normalization across backends; local backends need a
  user-installed runtime (documented in the operator guide / Settings).
- **Neutral:** decision quality differs between hosted Jev and open clones; the eval
  harness (spec R53 / task T44) measures this per workload.
