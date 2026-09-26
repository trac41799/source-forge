# Contributing to SourceForge

## Setup

```bash
npm install
npm run tauri dev     # full desktop shell
npm run dev           # frontend only (Vite)
```

Windows: the MSVC toolchain is required (`rust-toolchain.toml` pins `stable`).
Linux: install `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`,
`librsvg2-dev`, `patchelf`, `libssl-dev`.

## Before you push

Run the full gate set — CI runs the same commands:

```bash
npx tsc --noEmit
npm run lint:src
npx vitest run
cargo test --workspace
python -m pytest -q        # in local-daemon/ and webhook-server/
```

## IPC contract (required)

Every frontend `invoke()` call must go through a constant in
`src/lib/ipc/commands.ts`, and every command must be registered in
`src-tauri/src/lib.rs`. `src/__tests__/contracts/ipc-contract.test.ts` enforces:

1. no invoked command is missing from the registry,
2. every `IPC.*` key resolves,
3. no **new** inline `invoke("...")` literals beyond the recorded baseline,
4. the registered-but-unused set matches the snapshot.

Adding a command = register it, add an `IPC` constant, update the snapshot
deliberately (`UPDATE_IPC_SNAPSHOT=1 npx vitest run src/__tests__/contracts/ipc-contract.test.ts`).

## Wave/worktree workflow

Work is organised in waves (`wave/A` … `wave/E`). Each wave runs in its own
worktree:

```bash
git worktree add .worktrees/<name> -b wave/<letter>/<name>
```

One writer per file per wave; shared files (`lib.rs`, `commands.rs`, `db.rs`,
`App.tsx`, `Sidebar.tsx`, CI config) are owned by an integrator. New Tauri
commands live in their own module (e.g. `knowledge_commands.rs`) rather than
`commands.rs`.

## Migrations

Numbered `NNN_name.sql`, registered in `src-tauri/src/db.rs`. Numbers are
reserved per wave to avoid collisions; later migrations run non-fatally.

## Specs and plans

Design decisions live in `docs/2026-09-26-defect-closure/SPEC.md`; the TDD plan
and per-wave handoffs are alongside it under `handoffs/`.
