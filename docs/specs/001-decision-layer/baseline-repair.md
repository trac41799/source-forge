# Baseline repair — 2026-09-26

Recorded per SDD (constitution §5, checklist #24: a broken baseline blocks handoff).
This file documents how the pre-existing red baseline was diagnosed and repaired before
M0 implementation. It is **not** part of spec 001's feature scope.

## Symptom
`cargo test` could not compile/run and the frontend had 1 failing test:
- MSVC: compile errors (`stack_registry.rs` E0015; `spec_parser.rs` E0063).
- GNU (directory-default): test harness would not launch (`0xc000039 STATUS_ENTRYPOINT_NOT_FOUND`).
- `src-tauri/tests/integration_tests.rs`: references `agent_control_center::` / private modules.
- Frontend: `App.test.tsx > redirects /connectors → /integrations` failed.
- Working tree had uncommitted Rust changes that vanished mid-session.

## Root cause
`HEAD = afd234d` is a **broken mid-refactor commit**. A descendant commit
**`00fdde4` — "fix(wave-a): make repo build green — LazyLock stack registry, settings prefs
wiring, MSVC pin + CI, spec-parser markers, budget mapper + memory migration fixes"
(2026-09-26 19:16:15)** contained the repair, but it was reset away (unreachable/dangling),
and a parallel `git stash` (`f339727`, "On main: wave-A migration", 19:13:09) held the same
working-tree changes. `git status` confirmed the source files were back to broken `HEAD`.

## Repair (no history rewritten)
Restored the specific files from `00fdde4` (a descendant of HEAD) — behaviour-preserving fix,
not a feature change:

| File | Why |
|---|---|
| `src-tauri/src/stack_registry.rs` | `LazyLock<Vec<StackPreset>>` — non-const `.into()` in a `static` (E0015) |
| `src-tauri/src/spec_parser.rs` | phase-flush fix + marker parsing; `verify_deploy` test field |
| `src-tauri/src/verification.rs` | test fixtures (README/env/git remote; `import.meta.env` client) |
| `src-tauri/src/agent_adapters/mod.rs` | mock `parse_cost` also reads `usage.cost` |
| `src-tauri/src/handoff_parser.rs` | handoff fixture sections (`Completed Work`, `Test Results`) |
| `src-tauri/src/budget.rs` | budget mapper fix |
| `src-tauri/src/lib.rs` | `pub mod` exports required by integration tests |
| `src-tauri/tests/integration_tests.rs` | crate name `sourceforge::` (was `agent_control_center::`) |
| `src-tauri/migrations/011_memory.sql` | creates `memory_facts` |
| `docs/2026-09-26-defect-closure/TDD_PLAN.md` | fixture required by a spec-parser test |
| `src/**` (App.test, Settings, stores, Messages) + `local-daemon/tests/conftest.py` | frontend/daemon test green |

Plus a one-line `eslint.config.js` fix: ignore build artifacts across the repo
(`**/target/**`, `**/.next/**`, `.worktrees/**`) — `npm run lint` was linting `target/` and
`landing/.next/`.

## Verified baseline (post-repair)
- Rust (`cargo +stable-x86_64-pc-windows-msvc test`, `mingw64\bin` on PATH):
  **lib 142 + integration_tests 42 = 184 passed / 0 failed**.
- Frontend (`npm test`): **314 passed / 0 failed** (29 files).
- `npx tsc --noEmit`: clean. `npm run lint`: **0 errors** (14 pre-existing warnings).

## Notes / risks
- Use **MSVC explicitly**; the directory override defaults to GNU, which cannot launch the
  test harness on this box. `windres`/`dlltool` must be on PATH (`C:\Users\mrtra\tools\mingw64\bin`).
- `00fdde4` is a dangling commit; the repo would be healthier if it were restored/committed by
  its author rather than left unreachable.
