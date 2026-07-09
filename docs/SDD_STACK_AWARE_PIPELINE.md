# SDD: Stack-Aware Orchestration Pipeline

**Status**: Design Phase  
**Goal**: SourceForge must choose the right architecture stack, provision infrastructure, and verify E2E before declaring a wave complete.

## 1. Current State Audit

### What Exists
| Module | Capability |
|--------|-----------|
| `integrations.rs` | Supabase config CRUD, feature groups, project detection from `.env` |
| `integrations.rs` | GitHub repo detection, PR creation, issue browser, lockdown mode |
| `agents/configs.ts` | 9 agent types (Claude, OpenCode, Aider, Goose, Cline, Cursor, Gemini, Qwen, Codex) |
| `verification.rs` | 18 build/deploy/quality/E2E checks |
| `playbook.rs` | Export/import `.acc` bundles with skills, memory, presets |
| `skillbridge.rs` | macOS SkillBridge.app detection |
| MCP Server `supabase-sino-vn` | Connected to Supabase project `ucrqqqnwgcrtpkxagvil` via OpenCode config |

### What's Missing
1. **No stack definitions** — SourceForge doesn't know about Next.js, Vercel, Supabase as a stack
2. **No preferences system** — no DB table for user preferences, no UI
3. **No infrastructure provisioning** — can't install Vercel CLI, can't auto-create Supabase projects
4. **No architecture decision** — always generates Express+Vite, never considers deployment target
5. **No delegation** — when automation fails, no clear handoff to user

## 2. Stack Registry Design

### Supported Stacks (Priority Order)

```rust
StackPreset {
    id: "nextjs-supabase-vercel",       // Default
    name: "Next.js + Supabase + Vercel",
    description: "Full-stack with API routes, PostgreSQL, edge deployment",
    frameworks: ["next.js"],
    database: "supabase-postgresql",
    deploy_target: "vercel",
    required_cli: ["vercel", "node", "npm"],
    required_mcp: ["supabase"],
    scaffold: NextJsSupabaseScaffold { ... },
    package_json_template: "{ ... }",
}

StackPreset {
    id: "nextjs-supabase-fastapi",
    name: "Next.js + Supabase + FastAPI",
    required_cli: ["vercel", "node", "npm", "python3"],
    ...
}

StackPreset {
    id: "express-react-supabase",
    name: "Express + React + Supabase",
    ...
}
```

### CLI Detection
- Check PATH for: `node`, `npm`, `vercel`, `python3`, `pip`
- Check installed MCP servers via OpenCode config
- Report: installed/missing per stack

## 3. User Preferences Design

### Database Schema
```sql
CREATE TABLE user_preferences (
  id TEXT PRIMARY KEY,
  preferred_stack TEXT DEFAULT 'nextjs-supabase-vercel',
  default_deploy_target TEXT DEFAULT 'vercel',
  auto_provision BOOLEAN DEFAULT 1,
  created_at TEXT,
  updated_at TEXT
);
```

### Tauri Commands
- `get_preferences` → UserPreferences
- `set_preferences(prefs)` → UserPreferences
- `detect_installed_clis` → CLIStatus[]
- `list_available_stacks` → StackPreset[]

### Frontend
- New tab: **Settings > Stack** (or augment existing Settings page)
- Stack selector dropdown with descriptions
- CLI status indicators (green/red dots)
- "Save Preferences" button

## 4. Infrastructure Provisioner Design

### Flow
```
1. User selects stack → SourceForge checks CLI availability
2. Missing CLIs → SourceForge attempts install (npm install -g vercel)
3. Can't install → Delegate to user with instructions
4. All CLIs ready → Provision DB via MCP:
   a. Use supabase-sino-vn MCP to create tables
   b. Get connection URL → write to .env
5. Write deployment configs (vercel.json, supabase/config.toml)
```

### Provisioner Commands
- `provision_infrastructure(stack_id, project_path)` → ProvisionReport
- `install_cli(tool_name)` → bool
- `check_cli_status()` → CLIStatus[]

## 5. Architecture Decision Engine

### When creating a new wave plan:
1. Read user preferences → get preferred stack
2. If no preferences → prompt user to choose
3. Based on stack → select:
   - Framework (Next.js vs Express+Vite)
   - Project scaffold
   - Database ORM (Prisma with correct provider)
   - Deploy config template
4. Write scaffold to project directory
5. Continue with normal wave execution

## 6. Delegation Protocol

When SourceForge cannot automate a step:
```rust
DelegationTask {
    id: uuid,
    step: String,          // "Install Vercel CLI"
    instructions: String,  // "Run: npm install -g vercel"
    reason: String,        // "npm not found on PATH"
    status: DelegationStatus, // PENDING | COMPLETED | SKIPPED
}
```

## 7. File Plan

| File | Purpose |
|------|---------|
| `src-tauri/src/stack_registry.rs` | Stack definitions, CLI detection, stack recommendations |
| `src-tauri/src/preferences.rs` | UserPreferences struct, DB CRUD, Tauri commands |
| `src-tauri/src/provisioner.rs` | Infrastructure provisioning, CLI install, DB provisioning |
| `src-tauri/src/arch_engine.rs` | Architecture decision, scaffold generation per stack |
| `src-tauri/src/delegation.rs` | Delegation tasks, user instructions |
| `src-tauri/migrations/015_user_preferences.sql` | Preferences table |
| `src-tauri/src/commands.rs` | +15 new Tauri commands |
| `src-tauri/src/lib.rs` | +5 modules, +15 invoke_handler entries |
| `src/pages/Settings.tsx` | Add Stack tab to settings |
| `src/components/StackSelector.tsx` | Stack selection UI component |
| `src/stores/preferencesStore.ts` | Zustand store for preferences |
