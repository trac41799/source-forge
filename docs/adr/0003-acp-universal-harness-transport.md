# ADR 0003 — Universal ACP (Agent Client Protocol) transport for automated execution and ConPTY/openpty for interactive terminals

**Status:** Proposed · **Date:** 2026-11-20 · **Spec:** 002-acp-execution-spine · **Issues:** #19, #20

## Context
SourceForge (ACC) advertises unification of 9+ coding agents and multi-provider orchestration. However:
1. Automated wave execution (`src-tauri/src/agent_adapters/mod.rs`) only implements `opencode` (by spawning `cmd /C opencode run --format json --auto` and scraping JSON stdout lines) and `mock`.
2. Interactive terminal panels (`src-tauri/src/pty.rs`) use `tokio::process::Command` with `Stdio::piped()`, which is not a pseudo-terminal (`isatty()` is false), never emits `agent-output` events to the frontend, and fails Tauri IPC deserialization on `spawn_agent` (`spawnCmd` vs `command`).
3. Across the 2026 coding agent ecosystem, **ACP (Agent Client Protocol)** — JSON-RPC 2.0 over `stdio` — has become the standard editor/orchestrator-to-agent protocol ("LSP for coding agents"), supported natively or via official bridges by **Pi Agent** (`pi-acp`, unlocking 15+ LLM providers), **OpenCode** (`opencode acp`), **Gemini CLI** (`gemini --acp`), **Claude Code** (`@agentclientprotocol/claude-agent-acp`), **Codex CLI** (`@agentclientprotocol/codex-acp`), and **Goose**.

Maintaining 9 separate ad-hoc CLI flag templates and stdout regex scrapers violates Constitution §1 (Simplicity) and §2 (Anti-Abstraction without concrete call sites), whereas ACP provides a single, schema-typed JSON-RPC 2.0 contract across all major harnesses.

## Decision
Split agent process management into two purpose-built, standards-compliant transports:

1. **`AcpAgentAdapter` (`src-tauri/src/agent_adapters/acp.rs`) for Automated Wave & Pipeline Execution:**
   - Speaks **ACP (JSON-RPC 2.0 over newline-delimited `stdio`)**:
     - Client → Agent requests: `initialize`, `session/new` (passing `cwd` and workspace `mcpServers`), `session/prompt`, `session/cancel`.
     - Agent → Client notifications & requests: `session/update` (text chunks, tool calls, file diffs, token/cost telemetry), `session/request_permission` (evaluated against worktree scope policy), `fs/read_text_file`, `fs/write_text_file`.
   - Registers 5 concrete harness profiles out of the box (`pi`, `opencode`, `gemini`, `claude`, `codex`) using one `AcpAgentAdapter` implementation parameterized by `AcpHarnessProfile`.
   - Decrypts provider API keys from ACC's AES-256 Connector Vault (`src-tauri/src/assets.rs`) and injects them as environment variables into the spawned ACP child process, enabling zero-CLI-config multi-provider execution via `pi-acp` or `opencode acp`.
   - Retains `OpenCodeAdapter` (`opencode run --format json`) as a graceful degradation fallback (Constitution §6) when an older OpenCode CLI binary lacks the `acp` subcommand.

2. **`portable-pty` (`ConPTY` on Windows, `openpty` on Unix) in `src-tauri/src/pty.rs` for Interactive `/runner` Terminals:**
   - Replaces `Stdio::piped()` in `PtyManager` with a real pseudo-terminal pair (`portable_pty::native_pty_system()`).
   - Emits `agent-output` and `agent-status` Tauri events (`src-tauri/src/agent_events.rs`) from the PTY reader loop.
   - Supports `resize_pty(agent_id, cols, rows)` and raw keystroke streaming from `PtyTerminal.tsx` (`terminal.onData` → `write_to_agent`).

## Consequences
- **Positive:** One protocol implementation (`AcpAgentAdapter`) connects SourceForge to Pi Agent (15+ providers), OpenCode, Gemini CLI, Claude Code, and Codex CLI with structured tool calls, diffs, permission mediation, MCP pass-through, and cost telemetry. Interactive `/runner` terminals render full TUI apps (`claude`, `opencode`, `aider`) properly via `ConPTY`/`openpty`.
- **Negative:** Adds `portable-pty` crate dependency to `src-tauri/Cargo.toml` (justified by Constitution §1 as it replaces broken piped-stdio emulation and satisfies Epic 1).
- **Neutral:** Existing `HANDOFF_<id>.md` validation remains supported, and `AcpAgentAdapter` can synthesize a canonical `HANDOFF_<id>.md` from the structured ACP session response + worktree diff if the agent completes its ACP turn without writing the file manually.
