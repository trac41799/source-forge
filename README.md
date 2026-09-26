<p align="center">
  <img alt="SourceForge" src="app-icon.png" width="128" height="128">
</p>
<p align="center">
  <strong>The forge that makes your AI agents smarter the more you use them.</strong>
</p>
  <h1 align="center">SourceForge</h1>
  <p align="center">
    <strong>The cockpit that makes your AI agents smarter the more you use them.</strong>
  </p>
</p>

<p align="center">
  <a href="https://github.com/edge8/agent-control-center/releases"><img src="https://img.shields.io/badge/version-0.0.0--alpha-blueviolet?style=flat-square" alt="Version"></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="License"></a>
  <a href="https://tauri.app"><img src="https://img.shields.io/badge/built_with-Tauri_v2-ffc131?style=flat-square" alt="Tauri"></a>
  <a href="https://react.dev"><img src="https://img.shields.io/badge/frontend-React_19-61dafb?style=flat-square" alt="React"></a>
  <img src="https://img.shields.io/badge/platform-Windows_%7C_macOS_%7C_Linux-eee?style=flat-square" alt="Platforms">
  <br>
  <a href="https://github.com/edge8/agent-control-center/actions/workflows/ci.yml"><img src="https://github.com/edge8/agent-control-center/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/agents-9_supported-2dd4ff?style=flat-square" alt="9 Agents">
  <img src="https://img.shields.io/badge/stack-Rust_%7C_React_%7C_SQLite-ff6b6b?style=flat-square" alt="Stack">
</p>

<br>

<p align="center">
  <a href="README.md">English</a> · <a href="docs/README.zh.md">简体中文</a> · <a href="docs/README.vi.md">Tiếng Việt</a>
</p>

<br>

## ✨ Overview

**SourceForge** (formerly Agent Control Center) is a local-first desktop application that unifies **9 AI coding agents** — Claude Code, OpenCode, Aider, Goose, Cline CLI, Cursor, Gemini CLI, Qwen Code, and Codex CLI — into a single orchestrated interface.

Built with Tauri v2, ACC runs agents in parallel PTY sessions with wave-based execution, dependency-aware scheduling, and automated handoff verification. Every session feeds a built-in **Knowledge Compounder** that distills decisions, patterns, and lessons into structured, compounding learning materials — so your team's AI workflows get measurably smarter project after project.

<br>

## 🚀 Features

<table>
<tr>
<td width="33%">

### 🤖 Multi-Agent Runner
Spawn and control up to 9 AI coding agents in a single window with resizable, detachable PTY panels, real-time status chips, and preset command buttons.

</td>
<td width="33%">

### 🌊 Wave Orchestration
Define dependency-aware work item tables with parallel execution across waves. Agents unlock per-dependency with stall detection, handoff verification, and automated correction loops.

</td>
<td width="33%">

### 🧠 Knowledge Compounder
After every completed wave, a two-pass async pipeline extracts decisions, patterns, anti-patterns, and lessons into structured, deduplicated knowledge items — no human input required.

</td>
</tr>
<tr>
<td>

### 📦 Memory Layer
Browser, diff, and cross-agent sync for all memory files (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `CONVENTIONS.md`, etc.). Reactive memory capture detects project conventions from PTY output and prompts for one-click injection.

</td>
<td>

### 🔍 Codebase Exploration
Auto-detects project tech stack, test framework, and package manager from `package.json`, `pyproject.toml`, `Cargo.toml`, etc. Suggests relevant MCP servers and agent configurations.

</td>
<td>

### 🕸️ Knowledge Graph
Visualize extracted patterns, decisions, and their relationships (extends, confirms, contradicts) in an interactive Cytoscape graph.

</td>
</tr>
<tr>
<td>

### 🎯 Token Budget System
Proactively allocate, monitor, and enforce token budgets per agent per task. Captures structured WIP checkpoints before budget exhaustion so work resumes cleanly.

</td>
<td>

### 📋 Session Replay
Timeline browser showing every event across all agents: reads, edits, runs, corrections, handoffs, and outcomes. Export sessions as Markdown or PDF.

</td>
<td>

### 📦 Playbook Export/Import
Package your project's skills, memory, MCP configs, presets, and models into a portable `.acc` bundle. Import into any ACC instance in under 30 seconds.

</td>
</tr>
</table>

<br>

## 🧩 Supported Agents

| Agent | Install | Memory File | Subagents |
|---|---|---|---|
| **Claude Code** | `npm i -g @anthropic-ai/claude-code` | `CLAUDE.md` | ✅ |
| **OpenCode** | `npm i -g @opencode-ai/opencode` | `.opencode/memory/default.md` | ✅ |
| **Aider** | `pip install aider-chat` | `CONVENTIONS.md` | — |
| **Goose** | `brew install goose` | `.goose/instructions.md` | Roadmap |
| **Cline CLI** | `npm i -g cline` | `.clinerules` | ✅ |
| **Cursor** | [cursor.com](https://cursor.com) | `.cursor/rules` | ✅ |
| **Gemini CLI** | `npm i -g @google/gemini-cli` | `GEMINI.md` | ✅ |
| **Qwen Code** | `npm i -g @alibaba/qwen-code` | `qwen.md` | ✅ |
| **Codex CLI** | `npm i -g @openai/codex` | `AGENTS.md` | ✅ |

> All agents support full PTY read/write and wave orchestration. Cursor requires a Cursor subscription for auth.

<br>

## 📦 Installation

### Prerequisites

- **Node.js 18+** and **npm**
- **Rust toolchain** (stable, via [rustup](https://rustup.rs))
- At least **one AI coding agent CLI** installed (e.g., `claude`, `opencode`, `aider`)

### Download

Pre-built binaries are available on the [Releases](https://github.com/edge8/agent-control-center/releases) page.

### Build from Source

```bash
git clone https://github.com/edge8/agent-control-center.git
cd agent-control-center
npm install
npx tauri build
```

The compiled binary will be at `src-tauri/target/release/`.

### Platform Support

| Platform | Status |
|---|---|
| **Windows** (x64) | ✅ |
| **macOS** (Intel & Apple Silicon) | ✅ |
| **Linux** (x64) | ✅ |

<br>

## ⚡ Quick Start

```bash
# 1. Launch ACC — open the app, land on the Runner view
# 2. Open a project — select any local directory (or use the built-in demo)
# 3. Spawn an agent — click an agent button (e.g., "Claude Code") → PTY session starts
# 4. Type a task — watch the agent work in real time in the terminal panel
# 5. Go parallel — spawn a second agent; both run side by side
# 6. Check the Knowledge page — patterns and lessons extracted automatically
```

<br>

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      AGENT CONTROL CENTER                        │
│                                                                   │
│  ┌─────────────────────────┐    ┌──────────────────────────────┐  │
│  │    FRONTEND (React)     │    │     BACKEND (Tauri / Rust)    │  │
│  │                         │    │                              │  │
│  │  Agent Runner           │◄──►│  PTY Manager                 │  │
│  │  Wave Orchestrator      │    │  SQLite + WAL                │  │
│  │  Asset Manager          │    │  File Sync Engine            │  │
│  │  Outcome Dashboard      │    │  Connector Vault (AES-256)   │  │
│  │  Knowledge Panel        │    │  Knowledge Compounder        │  │
│  │  Playbook Manager       │    │  Cron Scheduler              │  │
│  │  Token Budgets          │    │  Token Guard + Monitor       │  │
│  └─────────────────────────┘    └──────────────────────────────┘  │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │               PYTHON SERVICES (optional)                    │   │
│  │  local-daemon   → file watchers + agent lifecycle          │   │
│  │  webhook-server → Slack / Lark / Jira webhook ingestion    │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │                     FILE SYSTEM                            │   │
│  │  ~/.claude/  ~/.opencode/  ~/.gemini/  ~/.codex/           │   │
│  │  ~/.aider/   ~/.goose/    ~/.cline/   ~/.cursor/           │   │
│  │  ~/.qwen/    Project directories                            │   │
│  └───────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**Stack:** [Tauri v2](https://tauri.app) (Rust) · [React 19](https://react.dev) · [Vite 6](https://vite.dev) · [Tailwind CSS v4](https://tailwindcss.com) · xterm.js · [Zustand](https://github.com/pmndrs/zustand) · SQLite · [shadcn/ui](https://ui.shadcn.com) · OpenRouter

<br>

## 💻 Development

```bash
# Start Vite dev server (frontend only)
npm run dev

# Start full Tauri desktop shell
npm run tauri dev

# Run tests (Vitest)
npm run test

# Lint
npm run lint

# Production build
npm run build
```

### Project Structure

```
agent-control-center/
├── src/                     # React frontend
│   ├── components/          # shadcn/ui components
│   ├── lib/                 # Agents, connectors, store, utils
│   ├── pages/               # Runner, Orchestrator, Knowledge, etc.
│   └── hooks/               # Custom React hooks
├── src-tauri/               # Rust backend
│   ├── src/                 # Tauri commands, PTY manager, DB, scheduler
│   ├── migrations/          # SQLite migrations
│   └── capabilities/        # Tauri capability permissions
├── local-daemon/            # Python file watchers & agent lifecycle
├── webhook-server/          # Python webhook ingestion server
├── docs/                    # Product & technical documentation
└── agent-workspaces/        # Isolated worktree directories for waves
```

<br>

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

<br>

## 🗺️ Roadmap

- [ ] Goose native subagent support
- [ ] Cloud sync for knowledge graphs across teams
- [ ] Custom agent SDK for third-party integrations
- [ ] VS Code extension for in-editor wave orchestration

<br>

## 📄 License

**MIT** © [Edge8](https://github.com/edge8)

<br>

---

<p align="center">
  <sub>Built with ACC. Every feature is a proof of concept of the product's own value.</sub>
  <br>
  <sub>⭐ Star us on GitHub — it helps more people discover ACC.</sub>
</p>
