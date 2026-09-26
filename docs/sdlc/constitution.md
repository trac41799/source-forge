# Project Constitution

Non-negotiable principles. Every spec, plan, and task is judged against these.
Adopted 2026-09-26. Changes require an ADR.

## 1. Simplicity
Prefer the smallest change that satisfies the requirement. No new runtime, service,
or dependency unless it replaces something larger. The decision layer is one module
with one interface — not a framework.

## 2. Anti-Abstraction
Do not introduce an abstraction until there are two concrete call sites. A single
interface over decision backends is justified as **intent**, not precedent: hosted Jev
(OpenRouter) and a local open-weight server (Von/Laya/Kev) are planned from the start as
two concrete implementations of the same `/v1/systemone` contract. If the local backend
is dropped, the abstraction collapses back to a single hosted client.

## 3. Integration-First
Reuse what exists: the OpenRouter call path, the `IntelligenceRequest` modes, the
SQLite schema, the existing UI. Extend in place; do not fork parallel pipelines.

## 4. Local-first / Privacy
The app must remain fully operable offline. A hosted decision backend is an
optional accelerator, never the only path. No user source, PTY output, or knowledge
content leaves the machine unless the user has enabled a hosted backend. Secrets
live in env/credential vault, never in source or client bundles.

## 5. Test-first
Every behavior begins as a failing test. The suite must be green before handoff.
New failures are triaged against the recorded baseline; no task may increase the
failure count.

## 6. Graceful Degradation
Every external dependency (hosted API, local model, network) has a specified
failure behavior: timeout, retry bound, and a defined fallback (existing
heuristic or an LLM call). A decision layer that fails must never block an agent
session — it degrades to the prior behavior and records why.
