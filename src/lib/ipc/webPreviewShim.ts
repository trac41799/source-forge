/**
 * Web-Preview Tauri Runtime Polyfill
 *
 * When SourceForge is opened in a regular browser (`npm run dev` at http://localhost:1420)
 * outside the native Tauri desktop webview, `window.__TAURI_INTERNALS__` is not injected
 * by the Rust host. Without a fallback, `@tauri-apps/api/core` and `@tauri-apps/api/event`
 * throw `TypeError: Cannot read properties of undefined (reading 'invoke' | 'transformCallback')`.
 *
 * This module installs a stateful in-memory / localStorage-backed preview shim ONLY when
 * `window.__TAURI_INTERNALS__` is absent, enabling full interactive UX & UAT exploration in
 * any browser while leaving native Tauri desktop execution untouched.
 */

interface PreviewWavePlan {
  id: string;
  project_id: string;
  slug: string;
  docs_path: string | null;
  status: string;
  created_at: string;
  completed_at: string | null;
}

interface PreviewPlanAgent {
  id: string;
  plan_id: string;
  agent_ref: string;
  task: string;
  wave: number;
  depends_on: string | null;
  agent_id: string | null;
  status: string;
  guideline_path: string | null;
  handoff_path: string | null;
  started_at: string | null;
  completed_at: string | null;
  retry_count: number;
}

export function installWebPreviewShim(): void {
  if (typeof window === "undefined") return;
  const win = window as unknown as Record<string, unknown>;
  if (win.__TAURI_INTERNALS__) return;

  const callbacks = new Map<number, (payload: unknown) => void>();
  const eventListeners = new Map<string, Set<number>>();
  let nextId = 1;

  const wavePlans: PreviewWavePlan[] = [
    {
      id: "plan-demo-acp-spine",
      project_id: "acc-main",
      slug: "acp-execution-spine",
      docs_path: "docs/specs/002-acp-execution-spine",
      status: "planning",
      created_at: new Date().toISOString(),
      completed_at: null,
    },
  ];

  const planAgents = new Map<string, PreviewPlanAgent[]>([
    [
      "plan-demo-acp-spine",
      [
        {
          id: "pa-1",
          plan_id: "plan-demo-acp-spine",
          agent_ref: "1-1",
          task: "Implement universal ACP JSON-RPC 2.0 adapter with worktree path jail",
          wave: 1,
          depends_on: null,
          agent_id: "pi",
          status: "done",
          guideline_path: ".acc/GUIDELINE.md",
          handoff_path: "HANDOFF_1-1.md",
          started_at: new Date().toISOString(),
          completed_at: new Date().toISOString(),
          retry_count: 0,
        },
        {
          id: "pa-2",
          plan_id: "plan-demo-acp-spine",
          agent_ref: "2-1",
          task: "Wire topological multi-wave DAG executor and FTS5 memory context injection",
          wave: 2,
          depends_on: "1-1",
          agent_id: "claude",
          status: "queued",
          guideline_path: ".acc/GUIDELINE.md",
          handoff_path: null,
          started_at: null,
          completed_at: null,
          retry_count: 0,
        },
      ],
    ],
  ]);

  const knowledgeItems = [
    {
      id: "ki-1",
      type: "pattern",
      title: "Universal ACP JSON-RPC 2.0 Transport over Stdio",
      content:
        "Drive Pi Agent (pi-acp), OpenCode (opencode acp), Gemini CLI (gemini --acp), Claude Code, and Codex CLI via one JSON-RPC 2.0 adapter with canonical worktree path containment.",
      tags: "acp,multi-provider,json-rpc",
      stack_tags: "rust,typescript,tauri",
      agent_tags: "pi,opencode,claude,codex,gemini",
      project_id: "acc-main",
      session_ids: "sess-001,sess-002",
      plan_ids: "plan-demo-acp-spine",
      confidence: 0.94,
      confirmation_count: 6,
      is_global: true,
      first_seen: "2026-11-20",
      last_confirmed: "2026-11-20",
      status: "confirmed",
    },
    {
      id: "ki-2",
      type: "antipattern",
      title: "Never Interpolate Raw Strings into SQLite Queries",
      content:
        "Always use rusqlite::params! or params_from_iter with ?N placeholders and FTS5 virtual tables instead of format! string interpolation.",
      tags: "security,sqlite,fts5",
      stack_tags: "rust,sqlite",
      agent_tags: "all",
      project_id: "acc-main",
      session_ids: "sess-002",
      plan_ids: "plan-demo-acp-spine",
      confidence: 0.98,
      confirmation_count: 9,
      is_global: true,
      first_seen: "2026-11-20",
      last_confirmed: "2026-11-20",
      status: "confirmed",
    },
  ];

  const memoryFacts = [
    {
      id: "mf-1",
      agent_id: "pi",
      session_id: "sess-001",
      user_id: "local-user",
      org_id: "default-org",
      fact_type: "decision",
      content: "Use Agent Client Protocol (ACP) as primary multi-provider harness transport and portable-pty for interactive TUI panels.",
      embedding: null,
      metadata: JSON.stringify(["AcpAgentAdapter", "PtyManager"]),
      confidence: 0.95,
      access_count: 4,
      last_accessed: new Date().toISOString(),
      created_at: new Date().toISOString(),
    },
    {
      id: "mf-2",
      agent_id: "claude",
      session_id: "sess-002",
      user_id: "local-user",
      org_id: "default-org",
      fact_type: "constraint",
      content: "Execute multi-wave DAG plans topologically: merge Wave N worktrees into base branch before creating Wave N+1 worktrees.",
      embedding: null,
      metadata: JSON.stringify(["wave_executor.rs", "worktree.rs"]),
      confidence: 0.92,
      access_count: 3,
      last_accessed: new Date().toISOString(),
      created_at: new Date().toISOString(),
    },
  ];

  const transformCallback = (cb?: (payload: unknown) => void, once = false): number => {
    const id = nextId++;
    if (cb) {
      callbacks.set(id, (payload: unknown) => {
        if (once) callbacks.delete(id);
        cb(payload);
      });
    }
    return id;
  };

  const unregisterCallback = (id: number): void => {
    callbacks.delete(id);
    for (const set of eventListeners.values()) {
      set.delete(id);
    }
  };

  const emitPreviewEvent = (event: string, payload: unknown): void => {
    const listeners = eventListeners.get(event);
    if (!listeners) return;
    for (const handlerId of listeners) {
      const fn = callbacks.get(handlerId);
      if (fn) {
        fn({ event, id: handlerId, payload });
      }
    }
  };

  const handleCommand = async (cmd: string, args: Record<string, unknown> = {}): Promise<unknown> => {
    // Tauri v2 event plugin protocol
    if (cmd === "plugin:event|listen") {
      const event = String(args.event ?? "");
      const handlerId = Number(args.handler ?? 0);
      if (!eventListeners.has(event)) {
        eventListeners.set(event, new Set());
      }
      eventListeners.get(event)!.add(handlerId);
      return handlerId;
    }
    if (cmd === "plugin:event|unlisten") {
      const event = String(args.event ?? "");
      const eventId = Number(args.eventId ?? 0);
      eventListeners.get(event)?.delete(eventId);
      return null;
    }
    if (cmd === "plugin:event|emit") {
      emitPreviewEvent(String(args.event ?? ""), args.payload);
      return null;
    }

    switch (cmd) {
      case "load_app_state":
        return { activeAgents: [], savedAt: new Date().toISOString() };
      case "save_app_state":
      case "clear_app_state":
        return null;

      case "check_agent_installed": {
        const agentId = String(args.agentId ?? "opencode");
        return {
          agent_id: agentId,
          installed: true,
          version: "1.16.0 (web-preview)",
          path: `/usr/local/bin/${String(args.command ?? agentId)}`,
          install_hint: null,
        };
      }

      case "spawn_agent":
      case "spawn_agent_with_guards": {
        const agentId = String(args.agentId ?? "opencode");
        const sessionId = `preview-${agentId}-${Date.now()}`;
        setTimeout(() => {
          emitPreviewEvent("agent-output", {
            agent_id: agentId,
            session_id: sessionId,
            line: `[SourceForge PTY · ${agentId}] Interactive session ready in ${String(args.projectPath ?? ".")}`,
            timestamp: new Date().toISOString(),
          });
        }, 40);
        return sessionId;
      }

      case "write_to_agent": {
        const agentId = String(args.agentId ?? "opencode");
        const text = String(args.text ?? "");
        if (text.trim()) {
          setTimeout(() => {
            emitPreviewEvent("agent-output", {
              agent_id: agentId,
              session_id: `preview-${agentId}`,
              line: `> ${text.replace(/\r?\n$/, "")}`,
              timestamp: new Date().toISOString(),
            });
          }, 20);
        }
        return null;
      }

      case "kill_agent":
      case "resize_pty_cmd":
        return null;

      case "get_wave_plans_cmd":
        return [...wavePlans];

      case "create_wave_plan_cmd": {
        const plan: PreviewWavePlan = {
          id: `plan-${Date.now()}`,
          project_id: String(args.projectId ?? "acc-main"),
          slug: String(args.slug ?? "new-wave"),
          docs_path: `docs/${new Date().toISOString().slice(0, 10)}/${String(args.slug ?? "new-wave")}`,
          status: "planning",
          created_at: new Date().toISOString(),
          completed_at: null,
        };
        wavePlans.unshift(plan);
        planAgents.set(plan.id, []);
        return plan;
      }

      case "get_plan_agents_cmd": {
        const planId = String(args.planId ?? "");
        return [...(planAgents.get(planId) ?? [])];
      }

      case "add_plan_agent_cmd": {
        const planId = String(args.planId ?? "");
        const list = planAgents.get(planId) ?? [];
        const added: PreviewPlanAgent = {
          id: `pa-${Date.now()}`,
          plan_id: planId,
          agent_ref: String(args.agentRef ?? `1-${list.length + 1}`),
          task: String(args.task ?? ""),
          wave: Number(args.wave ?? 1),
          depends_on: args.dependsOn ? String(args.dependsOn) : null,
          agent_id: args.agentId ? String(args.agentId) : "pi",
          status: "queued",
          guideline_path: ".acc/GUIDELINE.md",
          handoff_path: null,
          started_at: null,
          completed_at: null,
          retry_count: 0,
        };
        list.push(added);
        planAgents.set(planId, list);
        return added;
      }

      case "update_plan_agent_status_cmd": {
        const agentId = String(args.agentId ?? "");
        const status = String(args.status ?? "running");
        for (const list of planAgents.values()) {
          for (const a of list) {
            if (a.id === agentId) a.status = status;
          }
        }
        return null;
      }

      case "execute_wave_cmd": {
        const planId = String(args.planId ?? "");
        const list = planAgents.get(planId) ?? [];
        for (const a of list) {
          a.status = "done";
          a.completed_at = new Date().toISOString();
        }
        return {
          plan_id: planId,
          base_repo: String(args.baseRepo ?? "."),
          agents: list.map((a) => ({
            agent_ref: a.agent_ref,
            session_id: `acp-${a.agent_ref}`,
            worktree_path: `.worktrees/${planId}-${a.agent_ref}`,
            branch: `agent/${planId}-${a.agent_ref}`,
            status: "done",
            guideline_path: ".acc/GUIDELINE.md",
            cost_usd: 0.0042,
          })),
          started_at: new Date().toISOString(),
          completed_at: new Date().toISOString(),
          total_cost_usd: list.length * 0.0042,
        };
      }

      case "verify_and_finalize_wave_cmd":
        return {
          wave: args.report,
          verification: {
            passed: true,
            checks: [
              { name: "worktree_merge", status: "Pass", detail: "All agent branches merged cleanly" },
              { name: "build_and_test", status: "Pass", detail: "TypeScript & test gates green" },
            ],
          },
        };

      case "get_corrections_cmd":
      case "get_open_signals_cmd":
      case "get_memory_candidates_cmd":
      case "list_decision_reviews_cmd":
      case "kg_get_contradictions":
      case "kg_get_code_knowledge":
      case "kg_get_cochange_warnings":
        return [];

      case "get_knowledge_items_cmd":
      case "search_knowledge_cmd":
        return [...knowledgeItems];

      case "get_knowledge_stats_cmd":
        return {
          total_items: knowledgeItems.length,
          by_type: { pattern: 1, antipattern: 1 },
          avg_confidence: 0.96,
          confirmed_count: 2,
        };

      case "get_compounder_status_cmd":
        return {
          health: "healthy",
          last_run_at: new Date().toISOString(),
          items_extracted: knowledgeItems.length,
          last_error: null,
        };

      case "kg_get_subgraph":
      case "kg_get_subgraph_cmd":
      case "kg_local_search_cmd":
        return {
          nodes: knowledgeItems.map((item) => ({
            id: item.id,
            title: item.title,
            type: item.type,
            confidence: item.confidence,
            depth: 0,
          })),
          edges: [
            {
              from_id: "ki-1",
              to_id: "ki-2",
              relation_type: "confirms",
              trigram_tag: "qian",
              hexagram_tag: "qian-li",
              wuxing_cycle: "generative",
              bagua_confidence: 0.92,
            },
          ],
        };

      case "get_memory_facts_cmd":
        return [...memoryFacts];

      case "hybrid_search_memory_cmd":
        return memoryFacts.map((fact) => ({
          fact,
          score: 1.85,
          match_type: "fts5_bm25",
        }));

      case "get_memory_stats_cmd":
        return {
          total_facts: memoryFacts.length,
          total_checkpoints: 1,
          by_type: [
            { type: "decision", count: 1 },
            { type: "constraint", count: 1 },
          ],
          avg_confidence: 0.935,
          total_tokens_saved: 4200,
        };

      case "get_preferences_cmd":
        return {
          preferred_stack: "nextjs-supabase-vercel",
          default_deploy_target: "vercel",
          auto_provision: true,
        };

      case "set_preferences_cmd":
        return args.preferences ?? {
          preferred_stack: "nextjs-supabase-vercel",
          default_deploy_target: "vercel",
          auto_provision: true,
        };

      case "get_decision_config_cmd":
        return {
          backend: "hosted",
          base_url: "https://openrouter.ai/api",
          model: "typesafe/jev-1.13",
          accept_threshold: 0.75,
          review_threshold: 0.4,
          context_limit: 32000,
          timeout_ms: 5000,
        };

      case "decision_health_cmd":
        return {
          status: "healthy",
          backend: "hosted",
          model: "typesafe/jev-1.13",
          latency_ms: 84,
          last_success_ms: Date.now(),
          detail: "Web preview active",
        };

      case "get_coverage_stats_cmd":
        return {
          total_files: 46,
          mapped: 42,
          summarized: 38,
          analyzed: 35,
          unexplored: 4,
          coverage_pct: 91.3,
        };

      case "plugin:dialog|open": {
        const options = args.options as { directory?: boolean } | undefined;
        return options?.directory ? "D:/projects/demo-app" : "docs/PLAN.md";
      }

      default:
        return [];
    }
  };

  win.__TAURI_INTERNALS__ = {
    invoke: handleCommand,
    transformCallback,
    unregisterCallback,
    convertFileSrc: (filePath: string) => filePath,
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
    },
  };

  win.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (_event: string, eventId: number) => {
      unregisterCallback(eventId);
    },
  };
}
