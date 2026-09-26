import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { mockInvoke } from "../setup";

beforeEach(() => {
  useOrchestrationStore.setState({
    suggestions: [],
    models: [],
    wavePlans: [],
    planAgents: [],
    corrections: [],
    acbSignals: [],
    memoryCandidates: [],
    playbookManifest: null,
  });
});

describe("orchestrationStore", () => {
  describe("routing", () => {
    it("routeTask updates suggestions", async () => {
      const suggestions = [
        { agent_id: "claude", model_id: "claude-3", confidence: 0.8, reasoning: "best fit", success_rate: 0.9 },
      ];
      mockInvoke.mockResolvedValueOnce(suggestions);
      await useOrchestrationStore.getState().routeTask("fix bug", "debug");
      expect(useOrchestrationStore.getState().suggestions).toEqual(suggestions);
      expect(mockInvoke).toHaveBeenCalledWith("route_task_cmd", {
        taskDesc: "fix bug",
        taskType: "debug",
        projectId: undefined,
      });
    });

    it("getModels updates models", async () => {
      const models = [
        { id: "m1", label: "Claude", provider: "anthropic", model_path: "claude-3", strengths: null, agent_id: null, alternation_index: null, is_active: true },
      ];
      mockInvoke.mockResolvedValueOnce(models);
      await useOrchestrationStore.getState().getModels();
      expect(useOrchestrationStore.getState().models).toEqual(models);
    });

    it("addModel calls invoke without updating state", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      const entry = { id: "m2", label: "GPT", provider: "openai", model_path: "gpt-4", strengths: null, agent_id: null, alternation_index: null, is_active: true };
      await useOrchestrationStore.getState().addModel(entry);
      expect(mockInvoke).toHaveBeenCalledWith("add_model_cmd", { entry });
    });

    it("buildHandoff returns envelope string", async () => {
      const envelope = { original_task: "fix", completed_by: "claude", model_used: "claude-3", output_summary: "done", changed_files: ["a.ts"], diff_preview: "-old\n+new", handoff_instruction: "test", next_agent: "opencode", next_model: "gemini" };
      mockInvoke.mockResolvedValueOnce("## AGENT HANDOFF\n\nfix");
      const result = await useOrchestrationStore.getState().buildHandoff(envelope);
      expect(result).toContain("AGENT HANDOFF");
    });

    it("validateHandoff returns parsed result", async () => {
      mockInvoke.mockResolvedValueOnce([true, []]);
      const result = await useOrchestrationStore.getState().validateHandoff("## AGENT HANDOFF\n\n**From agent:** a\n**To agent:** b\n\n### Completed Work\ndone\n\n### Test Results\npass\n\n### Interface Contracts Exposed\nnone\n\n### Files NOT Modified\nnone\n\n### Design Decisions\nnone\n\n### Handoff Instructions\nnone");
      expect(result).toEqual({ valid: true, missing: [] });
    });
  });

  describe("orchestrator", () => {
    it("createWavePlan appends to wavePlans", async () => {
      const plan = { id: "wp-1", project_id: "proj-1", slug: "fix-auth", docs_path: null, status: "planning", created_at: "2026-06-01T00:00:00Z", completed_at: null };
      mockInvoke.mockResolvedValueOnce(plan);
      const result = await useOrchestrationStore.getState().createWavePlan("proj-1", "fix-auth");
      expect(result).toEqual(plan);
      expect(useOrchestrationStore.getState().wavePlans).toEqual([plan]);
    });

    it("getPlanAgents updates planAgents", async () => {
      const agents = [{ id: "pa-1", plan_id: "wp-1", agent_ref: "W1.A", task: "setup", wave: 1, depends_on: null, agent_id: null, status: "queued", guideline_path: null, handoff_path: null, started_at: null, completed_at: null, retry_count: 0 }];
      mockInvoke.mockResolvedValueOnce(agents);
      await useOrchestrationStore.getState().getPlanAgents("wp-1");
      expect(useOrchestrationStore.getState().planAgents).toEqual(agents);
    });

    it("generateGuideline returns guideline string", async () => {
      mockInvoke.mockResolvedValueOnce("AGENT W1.A GUIDELINE\n\nObjective: setup");
      const result = await useOrchestrationStore.getState().generateGuideline("W1.A", "setup", "Initial setup", undefined, ["claude"], ["src/main.rs"], []);
      expect(result).toContain("W1.A GUIDELINE");
    });

    it("createCorrection calls invoke", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await useOrchestrationStore.getState().createCorrection("wp-1", "W1.A", "bug", "root", "fix", "test", 1);
      expect(mockInvoke).toHaveBeenCalledWith("create_correction_cmd", { planId: "wp-1", agentRef: "W1.A", bugDesc: "bug", rootCause: "root", fixRequired: "fix", testRequired: "test", retryNumber: 1 });
    });
  });

  describe("ACB", () => {
    it("parseAcbSignal returns signal or null", async () => {
      mockInvoke.mockResolvedValueOnce({ id: "sig-1", session_id: "s1", wave: 1, from_agent: "A", to_agent: "B", signal_type: "BLOCKER", priority: "HIGH", body: "blocked", ref_id: null, status: "OPEN", created_at: "2026-06-01T00:00:00Z", resolved_at: null });
      const result = await useOrchestrationStore.getState().parseAcbSignal("[ACC:BLOCKER from=A to=B priority=HIGH] blocked");
      expect(result!.signal_type).toBe("BLOCKER");
    });

    it("recordAcbSignal appends to acbSignals", async () => {
      const signal = { id: "sig-2", session_id: "s1", wave: 1, from_agent: "A", to_agent: "B", signal_type: "INFO", priority: "INFO", body: "msg", ref_id: null, status: "OPEN", created_at: "2026-06-01T00:00:00Z", resolved_at: null };
      mockInvoke.mockResolvedValueOnce(undefined);
      await useOrchestrationStore.getState().recordAcbSignal(signal);
      expect(useOrchestrationStore.getState().acbSignals).toEqual([signal]);
    });

    it("resolveSignal removes from acbSignals", async () => {
      useOrchestrationStore.setState({
        acbSignals: [{ id: "sig-3", session_id: "s1", wave: 1, from_agent: "A", to_agent: "B", signal_type: "BLOCKER", priority: "HIGH", body: "x", ref_id: null, status: "OPEN", created_at: "2026-06-01T00:00:00Z", resolved_at: null }],
      });
      mockInvoke.mockResolvedValueOnce(undefined);
      await useOrchestrationStore.getState().resolveSignal("sig-3");
      expect(useOrchestrationStore.getState().acbSignals).toEqual([]);
    });
  });

  describe("playbook", () => {
    it("buildPlaybookManifest updates playbookManifest", async () => {
      const manifest = { version: "1.0", name: "test", project: "proj", exported_at: "2026-06-01T00:00:00Z", stacks: ["react"], includes: ["memory"] };
      mockInvoke.mockResolvedValueOnce(manifest);
      await useOrchestrationStore.getState().buildPlaybookManifest("test", "proj", ["react"], true, true, false);
      expect(useOrchestrationStore.getState().playbookManifest).toEqual(manifest);
    });
  });

  describe("verification gate (SPEC-001 §5 DG-4)", () => {
    it("finalizeWave calls verify_and_finalize_wave_cmd with report and projectPath", async () => {
      const report = {
        plan_id: "plan-1",
        base_repo: "/repo",
        agents: [],
        started_at: "2026-09-26T00:00:00Z",
        completed_at: null,
        total_cost_usd: 0,
      };
      const verification = {
        passed: false,
        checks: [{ name: "SPA rewrite excludes /api", status: { Fail: "missing" }, detail: "" }],
      };
      mockInvoke.mockResolvedValueOnce({ wave: report, verification });

      const result = await useOrchestrationStore
        .getState()
        .finalizeWave(report, "/repo/project");

      expect(mockInvoke).toHaveBeenCalledWith("verify_and_finalize_wave_cmd", {
        report,
        projectPath: "/repo/project",
      });
      expect(result.verification.passed).toBe(false);
      expect(result.wave.plan_id).toBe("plan-1");
    });
  });
});
