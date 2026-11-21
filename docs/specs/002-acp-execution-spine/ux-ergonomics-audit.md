# UI/UX Ergonomics Audit — Spec 002 (UX-01 through UX-13)

**Date:** 2026-11-20  
**Scope:** Full frontend audit of raw path inputs, freeform IDs, JSON textareas, and unwired buttons across `src/pages/` (`docs/spec-002-acp-spine` baseline `fbf0712`).

---

## Summary

| ID | Surface / File | Location | Category | Batch | Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **UX-01** | `src/pages/BuildApp.tsx` | L211–L218 | Raw Project Path Input | Batch 1 (`ux-picker-core`) | Fixed in `feat/ux-1-core` |
| **UX-02** | `src/pages/BuildApp.tsx` | L203–L210 | Raw Spec File Input | Batch 1 (`ux-picker-core`) | Fixed in `feat/ux-1-core` |
| **UX-03** | `src/pages/Runner.tsx` | L144–L157 | Missing Folder Browser in Project Bar | Batch 1 (`ux-picker-core`) | Fixed in `feat/ux-1-core` |
| **UX-04** | `src/pages/Settings.tsx` | L218–L228 | Raw Default Project Path Input | Batch 1 (`ux-picker-core`) | Fixed in `feat/ux-1-core` |
| **UX-05** | `src/pages/Assets.tsx` | L173, L190, L207, L232 | Raw Toolbar Folder/File Path Inputs | Batch 3 (`ux-assets-knowledge`) | Fixed in `feat/ux-3-assets-knowledge` |
| **UX-06** | `src/pages/Assets.tsx` | L765, L797–L804 | Raw Vault Agent Scope & Secret Key Entry | Batch 3 (`ux-assets-knowledge`) | Fixed in `feat/ux-3-assets-knowledge` |
| **UX-07** | `src/pages/Integrations.tsx` | L368–L374, L1163–L1167 | Raw Project Path & Daemon Config Path | Batch 3 (`ux-assets-knowledge`) | Fixed in `feat/ux-3-assets-knowledge` |
| **UX-08** | `src/pages/Orchestrate.tsx` | L373–L408 | Raw Agent ID, Depends On & Wave # Inputs | Batch 2 (`ux-orchestrate-route`) | Fixed in `feat/ux-2-orchestrate` |
| **UX-09** | `src/pages/Handoffs.tsx` | L84–L110, L173–L188 | Raw Agent Names & Missing Handoff File Picker | Batch 2 (`ux-orchestrate-route`) | Fixed in `feat/ux-2-orchestrate` |
| **UX-10** | `src/pages/Playbooks.tsx` | L122–L124, L160–L161 | Unwired `.acc` Picker & Reactive Memory Buttons | Batch 2 (`ux-orchestrate-route`) | Fixed in `feat/ux-2-orchestrate` |
| **UX-11** | `src/pages/Route.tsx` | L63–L69, L166–L169 | Raw Project ID & Unwired `Send to Agent` | Batch 2 (`ux-orchestrate-route`) | Fixed in `feat/ux-2-orchestrate` |
| **UX-12** | `src/pages/Knowledge.tsx` | L587, L1479, L1532, L1922 | Raw Agent Filter, File Paths & Compounder UUID | Batch 3 (`ux-assets-knowledge`) | Fixed in `feat/ux-3-assets-knowledge` |
| **UX-13** | `src/pages/Scheduler.tsx` | L834–L849 | Raw Wave Preset & Hand-Typed Escalation JSON | Batch 3 (`ux-assets-knowledge`) | Fixed in `feat/ux-3-assets-knowledge` |

---

## Core Foundation (Batch 1 — `ux-picker-core`)

To eliminate OS-path typing friction across the desktop app and keep browser web-preview (`npm run dev`) functional:

1. **Reusable Path Pickers (`src/components/ui/path-picker.tsx`)**
   - `<FolderPickerInput />`: Combines a path `<Input />`, an optional quick-select `<select>` populated from `recentPaths` / `currentProject?.path` when available, and a **`Browse...`** button (`FolderOpen` icon) backed by `open({ directory: true, multiple: false })` from `@tauri-apps/plugin-dialog`.
   - `<FilePickerInput />`: Combines a path `<Input />` and a **`Browse...`** button (`FileText` icon) backed by `open({ directory: false, multiple: false, filters })` from `@tauri-apps/plugin-dialog`.
2. **Web-Preview Dialog Shim (`src/lib/ipc/webPreviewShim.ts`)**
   - Handles `plugin:dialog|open` in `handleCommand` so browser preview clicks return `"D:/projects/demo-app"` when `options.directory` is true and `"docs/PLAN.md"` for file dialogs instead of falling through to the default `[]`.

---

## Detailed Findings & Remediations

### UX-01 — `BuildApp.tsx` Project Path Text Input
- **Location:** `src/pages/BuildApp.tsx` (L211–L218)
- **Problem:** `Project path` required typing an absolute OS directory path into a plain `<Input placeholder="/path/to/project" />`.
- **Remediation:** Replaced with `<FolderPickerInput browseLabel="Browse Folder..." />` while preserving the `"Project path"` label association and `"/path/to/project"` placeholder.

### UX-02 — `BuildApp.tsx` Spec File Text Input
- **Location:** `src/pages/BuildApp.tsx` (L203–L210)
- **Problem:** `Spec file (markdown)` required manually typing a relative or absolute markdown path into `<Input placeholder="docs/PLAN.md" />`.
- **Remediation:** Replaced with `<FilePickerInput browseLabel="Browse Spec..." filters={[{ name: "Markdown", extensions: ["md", "markdown", "txt"] }]} />` while preserving `"Spec file (markdown)"` label association and `"docs/PLAN.md"` placeholder.

### UX-03 — `Runner.tsx` Project Selector Lacks Folder Browser
- **Location:** `src/pages/Runner.tsx` (L144–L157)
- **Problem:** The top `Project:` bar only offered a `<select>` of `recentPaths`. On a fresh session (or when opening a new repository), users had no way to browse for a project folder from `/runner`.
- **Remediation:** Added a `Browse Folder...` button next to the `Project:` `<select>` that invokes `open({ directory: true, multiple: false, title: "Select Project Folder" })` and calls `switchProject(selected)`.

### UX-04 — `Settings.tsx` Default Project Path Text Input
- **Location:** `src/pages/Settings.tsx` (L218–L228)
- **Problem:** `Default Project Path` was a plain `<Input placeholder="/path/to/project" />` without a native folder picker.
- **Remediation:** Replaced with `<FolderPickerInput id="project-path" placeholder="/path/to/project" browseLabel="Browse Folder" />` with an explicit `<label htmlFor="project-path">` association.

### UX-05 — `Assets.tsx` Raw Toolbar Folder & File Inputs
- **Location:** `src/pages/Assets.tsx` (L173 `Skills` scan path, L190 `Memory` project path, L207 `MCPs` config path, L232 `Plugins` project path)
- **Problem:** All four asset-tab toolbars used raw text inputs without native folder/file pickers or automatic active-project path defaults.
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Swap folder inputs (`Skills`, `Memory`, `Plugins`) to `<FolderPickerInput />` and `MCPs` config path to `<FilePickerInput />` (`.json` / `.toml` / `.yaml`), defaulting from `useProjectStore((s) => s.currentProject?.path)`.

### UX-06 — `Assets.tsx` Vault Secret Key & Agent Scope Friction
- **Location:** `src/pages/Assets.tsx` (L765 `Key Name`, L797–L804 `Agent ID (optional)`)
- **Problem:** Adding a secret to the Connector Vault required manually typing `Agent ID (optional)` (`placeholder="e.g. claude-code"`) and offered no quick-pick chips for standard provider environment keys (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`).
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Replace `Agent ID` with a `<select>` populated from `AGENT_CONFIGS` + `pi` (`All Agents` default) and add one-click provider key chips that populate `Key Name`.

### UX-07 — `Integrations.tsx` Raw Project Path & Daemon Config Path
- **Location:** `src/pages/Integrations.tsx` (L368–L374 `Project Path`, L1163–L1167 daemon `Config Path`)
- **Problem:** Both `Project Path` (`placeholder="/path/to/project"`) and daemon `Config Path` (`placeholder="/path/to/daemon/config.yaml"`) required manual path typing.
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Replace `Project Path` with `<FolderPickerInput />` and daemon `Config Path` with `<FilePickerInput />` filtered for YAML/JSON/TOML configs.

### UX-08 — `Orchestrate.tsx` Raw Agent ID, Depends On & Wave # Inputs
- **Location:** `src/pages/Orchestrate.tsx` (L373–L408 `Add Plan Agent`)
- **Problem:** `Agent ID (optional)` (`placeholder="e.g., claude-code"`), `Depends On` (`placeholder="agent ref"`), and `Wave #` were raw text inputs, inviting typos, unknown harness IDs, and broken DAG references.
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Replace `Agent ID` with a `<select aria-label="Agent ID">` listing all ACP/CLI harnesses (`pi`, `opencode`, `opencode-acp`, `claude`, `codex`, `gemini`, `aider`, `goose`, `cline`, `cursor`, `qwen-code`), replace `Depends On` with a `<select aria-label="Depends On">` listing existing `store.planAgents` in the selected plan, change `Wave #` to `<Input type="number" min={1} max={20} />`, and auto-default `Project ID` from `currentProject?.id || "acc-main"`.

### UX-09 — `Handoffs.tsx` Raw Agent Inputs & Missing Handoff File Picker
- **Location:** `src/pages/Handoffs.tsx` (L84–L110 `Completed By` / `Next Agent`, L173–L188 `Validate Handoff Schema`)
- **Problem:** `Completed By` and `Next Agent` were raw text inputs, and `Validate Handoff Schema` forced users to copy/paste markdown into a `<textarea>` with no file picker to load `HANDOFF_*.md` files from disk.
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Replace `Completed By` and `Next Agent` with agent dropdowns populated from `AGENT_CONFIGS` + `pi`, and add a `Browse Handoff File...` button in `Validate Handoff Schema` using `open({ directory: false, filters: [{ name: "Markdown", extensions: ["md", "markdown"] }] })`.

### UX-10 — `Playbooks.tsx` Unwired `.acc` File Picker & Reactive Memory Actions
- **Location:** `src/pages/Playbooks.tsx` (L122–L124 `Select .acc File`, L160–L161 `Add to Memory` / `Skip`)
- **Problem:** `Select .acc File` had no `onClick` handler and no file dialog integration, and the `Add to Memory` and `Skip` buttons on Reactive Memory candidate cards had no `onClick` handlers.
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Wire `Select .acc File` to `open({ directory: false, filters: [{ name: "ACC Playbook", extensions: ["acc", "json", "zip"] }] })` with selected path/import confirmation state, and wire `Add to Memory` / `Skip` handlers to update candidate status and provide feedback.

### UX-11 — `Route.tsx` Raw Project ID & Unwired `Send to Agent` Button
- **Location:** `src/pages/Route.tsx` (L63–L69 `Project ID (optional)`, L166–L169 `Send to Agent`)
- **Problem:** `Project ID (optional)` forced typing a raw project identifier, and `SuggestionCard`'s `Send to Agent` button had no `onClick` handler.
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Replace `Project ID (optional)` with a `<select aria-label="Project Scope">` offering `"All Projects (Cross-project routing)"` plus active/recent projects, and wire `Send to Agent` to spawn/select the recommended agent via `useAgentStore` and navigate to `/runner`.

### UX-12 — `Knowledge.tsx` Raw Agent Filter, File Paths & Compounder UUID
- **Location:** `src/pages/Knowledge.tsx` (L587 `Agent` filter, L1479 `Code Bridge` file path, L1532 `Co-change` file path, L1922 `Run Compounder` session ID)
- **Problem:** Filtering by agent, checking `Code Bridge` / `Co-change` file paths, and running the knowledge compounder required manually typing agent strings, relative file paths, and raw session UUIDs (`placeholder="e.g., 5a4b..."`).
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Use an agent selector for the filter, `<FilePickerInput />` for `Code Bridge` and `Co-change` path inputs, and a session selector/recent-sessions dropdown for `Run Compounder`.

### UX-13 — `Scheduler.tsx` Raw Wave Preset & Hand-Typed Escalation JSON
- **Location:** `src/pages/Scheduler.tsx` (L834–L849 `Wave Preset`, `Escalation Policy (JSON)`)
- **Problem:** `Wave Preset` was a raw text input (`placeholder="e.g., default-4-wave"`), and `Escalation Policy (JSON)` forced users to hand-type raw JSON (`placeholder='{"max_retries": 3, "retry_delay_minutes": 30}'`).
- **Remediation (original plan; see the branch tests for the as-built controls, which in places use `Browse ...` buttons, chips, or `<select>` instead of the exact component named):** Replace `Wave Preset` with a `<select>` of available wave presets / plans, and replace raw JSON editing with structured numeric/toggle form fields (max retries, retry delay minutes, escalation action) that serialize to valid JSON.
