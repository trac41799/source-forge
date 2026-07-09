# TDD Plan: Stack-Aware Orchestration Pipeline

**Principle**: RED → GREEN → REFACTOR. Write failing tests first, implement minimal code to pass, refactor.

## Phase 1: Stack Registry (RED first)

### Tests (`stack_registry.rs`)
```
test_stacks_have_required_fields       — Every StackPreset has id, name, frameworks, required_cli
test_default_stack_is_nextjs_supabase  — First stack in registry is nextjs-supabase-vercel
test_cli_detection_finds_node          — Detects node on PATH (always present in dev)
test_cli_detection_missing_tool        — Returns Missing for non-existent CLI
test_stack_recommendation_by_clis      — Based on available CLIs, recommend best stack
test_stack_by_id_lookup                — get_stack("nextjs-supabase-vercel") returns Some
test_all_stack_ids_unique              — No duplicate stack IDs
```

### Implementation
- `StackPreset` struct with all fields
- `STACK_REGISTRY: &[StackPreset]` — compile-time constant
- `detect_installed_clis() -> HashMap<String, CliStatus>`
- `recommend_stack(cli_status) -> &StackPreset`
- `get_stack(id) -> Option<&StackPreset>`

---

## Phase 2: User Preferences (RED first)

### Tests (`preferences.rs`)
```
test_default_preferences               — Default is nextjs-supabase-vercel, auto_provision=true
test_set_and_get_preferences           — Write prefs to DB, read back, values match
test_preferences_json_roundtrip        — Serialize → deserialize preserves all fields
test_invalid_stack_id_rejected         — Setting invalid stack_id returns error
test_missing_db_returns_defaults       — When no row exists, returns defaults
```

### Implementation
- `UserPreferences` struct (Serialize, Deserialize)
- `get_preferences(db) -> UserPreferences`
- `set_preferences(db, prefs) -> Result<()>`
- Migration `015_user_preferences.sql`
- Tauri commands in `commands.rs`

---

## Phase 3: Infrastructure Provisioner (RED first)

### Tests (`provisioner.rs`)
```
test_cli_status_report                 — check_cli_status returns correct installed/missing
test_provision_report_structure        — ProvisionReport has all required fields
test_stack_requirements_resolved       — Resolves which CLIs are needed for a stack
test_missing_cli_list                  — Returns list of CLIs that need installation
```

### Implementation
- `ProvisionReport` struct
- `check_cli_status() -> Vec<CliStatus>`
- `resolve_stack_requirements(stack_id) -> StackRequirements`
- `provision_infrastructure(stack_id, project_path) -> ProvisionReport`
- Tauri commands

---

## Phase 4: Architecture Engine (RED first)

### Tests (`arch_engine.rs`)
```
test_scaffold_nextjs_generates_package_json    — package.json has next, react, prisma deps
test_scaffold_nextjs_generates_next_config     — next.config.js/ts exists
test_scaffold_nextjs_generates_prisma_schema   — prisma/schema.prisma with postgresql provider
test_scaffold_nextjs_generates_api_routes      — app/api/ directory exists
test_scaffold_express_generates_different_files — Express stack creates different scaffold
test_scaffold_respects_project_name            — package.json name matches project
```

### Implementation
- `scaffold_project(stack_id, project_path, project_name) -> ScaffoldReport`
- Template files embedded as `include_str!`
- Stack-specific scaffolding logic

---

## Phase 5: Delegation Protocol (RED first)

### Tests (`delegation.rs`)
```
test_create_delegation_task             — Creates task with instructions
test_delegation_task_status_transitions — PENDING → COMPLETED
test_delegation_report_summary          — Summary shows pending/completed counts
```

### Implementation
- `DelegationTask` struct
- `DelegationReport` struct
- `create_delegation(step, instructions, reason) -> DelegationTask`
- Tauri command

---

## Phase 6: Integration (GREEN)

### Integration Tests
```
test_full_pipeline_nextjs_stack
  - Set preferences to nextjs-supabase-vercel
  - Scaffold project
  - Verify all expected files exist
  - Check package.json has correct deps
  - Check prisma schema uses postgresql

test_full_pipeline_cli_detection
  - Detect CLIs
  - Recommend stack based on available CLIs
  - Verify recommendation is valid
```

---

## Phase 7: Frontend (RED first)

### Component Tests
```
test_stack_selector_renders_options     — Renders list of available stacks
test_stack_selector_calls_on_change     — Selecting fires callback
test_cli_status_indicator_shows_green   — Installed CLI shows green dot
test_cli_status_indicator_shows_red     — Missing CLI shows red dot
test_preferences_page_saves             — Save button persists to backend
```

---

## Phase 8: Regression Validation

```
verify_308_js_tests_pass    — All existing tests still pass
verify_cargo_check_clean    — No compilation errors (only dlltool pre-existing)
verify_smoke_tests_pass     — App launches, tabs navigate
```
