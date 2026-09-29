import { useIntelligenceStore } from "@/stores/intelligenceStore";
import { mockInvoke } from "../setup";

const sampleRecord = {
  id: "rec-1",
  session_id: "s1",
  agent_id: "agent-1",
  task_type: "review",
  outcome: "done",
  duration_s: 30,
  created_at: "2026-06-01T00:00:00Z",
};

const sampleStats = [
  {
    agent_id: "agent-1",
    task_type: "review",
    project_id: null,
    total: 10,
    done: 8,
    failed: 1,
    revised: 1,
    avg_duration_s: 45,
    success_rate: 0.8,
  },
];

beforeEach(() => {
  useIntelligenceStore.setState({
    outcomeStats: [],
    failureAnalyses: [],
    limitEvents: [],
    tokenStats: null,
    heartbeatResults: [],
  });
});

describe("intelligenceStore", () => {
  it("recordOutcome calls invoke and returns result", async () => {
    mockInvoke.mockResolvedValueOnce(sampleRecord);
    const result = await useIntelligenceStore
      .getState()
      .recordOutcome("s1", "agent-1", "review", "done", 30);
    expect(result).toEqual(sampleRecord);
    expect(mockInvoke).toHaveBeenCalledWith("record_outcome_cmd", {
      sessionId: "s1",
      agentId: "agent-1",
      taskType: "review",
      outcome: "done",
      durationS: 30,
    });
  });

  it("inferOutcome calls the decision-based infer command", async () => {
    mockInvoke.mockResolvedValueOnce(sampleRecord);
    const result = await useIntelligenceStore
      .getState()
      .inferOutcome("s1", "agent-1", "review", "all tests passed", 5, 30);
    expect(result).toEqual(sampleRecord);
    expect(mockInvoke).toHaveBeenCalledWith("infer_outcome_cmd", {
      sessionId: "s1",
      agentId: "agent-1",
      taskType: "review",
      ptyOutput: "all tests passed",
      idleSeconds: 5,
      durationS: 30,
    });
  });

  it("diagnoseFailure calls the decision-based diagnose command", async () => {
    mockInvoke.mockResolvedValueOnce(0.8);
    const confidence = await useIntelligenceStore
      .getState()
      .diagnoseFailure("fa-1", "null pointer", "missing check", "add null guard");
    expect(confidence).toBe(0.8);
    expect(mockInvoke).toHaveBeenCalledWith("diagnose_failure_cmd", {
      analysisId: "fa-1",
      diagnosis: "null pointer",
      rootCause: "missing check",
      suggestedFix: "add null guard",
    });
  });

  it("getOutcomeStats updates state from invoke", async () => {
    mockInvoke.mockResolvedValueOnce(sampleStats);
    await useIntelligenceStore.getState().getOutcomeStats();
    expect(useIntelligenceStore.getState().outcomeStats).toEqual(sampleStats);
  });

  it("createFailureAnalysis prepends to list", async () => {
    const analysis = {
      id: "fa-1",
      session_id: "s1",
      pty_excerpt: "Error: crash",
      diagnosis: "null pointer",
      root_cause: "missing check",
      suggested_fix: "add null guard",
      confidence: 0.8,
      created_at: "2026-06-01T00:00:00Z",
    };
    mockInvoke.mockResolvedValueOnce(analysis);
    const result = await useIntelligenceStore
      .getState()
      .createFailureAnalysis("s1", "Error: crash");
    expect(result).toEqual(analysis);
    expect(useIntelligenceStore.getState().failureAnalyses).toEqual([analysis]);
  });

  it("detectLimitEvent returns parsed result", async () => {
    mockInvoke.mockResolvedValueOnce(["rate_limit", "rate limit"]);
    const result = await useIntelligenceStore
      .getState()
      .detectLimitEvent("Error: rate limit exceeded");
    expect(result).toEqual(["rate_limit", "rate limit"]);
    expect(mockInvoke).toHaveBeenCalledWith("detect_limit_event_cmd", {
      rawOutput: "Error: rate limit exceeded",
    });
  });

  it("recordLimitEvent appends to limitEvents", async () => {
    const event = {
      id: "le-1",
      session_id: "s1",
      plan_agent_id: null,
      event_type: "rate_limit",
      raw_message: "too fast",
      resolved: false,
      resolved_at: null,
      resolution: null,
    };
    mockInvoke.mockResolvedValueOnce(event);
    const result = await useIntelligenceStore
      .getState()
      .recordLimitEvent("s1", null, "rate_limit", "too fast");
    expect(result).toEqual(event);
    expect(useIntelligenceStore.getState().limitEvents).toEqual([event]);
  });

  it("resolveLimitEvent marks event as resolved", async () => {
    useIntelligenceStore.setState({
      limitEvents: [
        {
          id: "le-1",
          session_id: "s1",
          plan_agent_id: null,
          event_type: "rate_limit",
          raw_message: "too fast",
          resolved: false,
          resolved_at: null,
          resolution: null,
        },
      ],
    });
    mockInvoke.mockResolvedValueOnce(undefined);
    await useIntelligenceStore.getState().resolveLimitEvent("le-1", "switched model");
    expect(
      useIntelligenceStore.getState().limitEvents[0].resolved
    ).toBe(true);
    expect(
      useIntelligenceStore.getState().limitEvents[0].resolution
    ).toBe("switched model");
    expect(mockInvoke).toHaveBeenCalledWith("resolve_limit_event_cmd", {
      eventId: "le-1",
      resolution: "switched model",
    });
  });

  it("getUnresolvedLimits replaces limitEvents", async () => {
    const events = [
      {
        id: "le-2",
        session_id: "s1",
        plan_agent_id: null,
        event_type: "timeout",
        raw_message: "slow",
        resolved: false,
        resolved_at: null,
        resolution: null,
      },
    ];
    mockInvoke.mockResolvedValueOnce(events);
    await useIntelligenceStore.getState().getUnresolvedLimits("s1");
    expect(useIntelligenceStore.getState().limitEvents).toEqual(events);
  });

  it("recordTokenUsage returns result without updating state", async () => {
    const usage = {
      id: "tu-1",
      session_id: "s1",
      agent_id: null,
      context: "chat",
      model: "claude-3",
      tokens_in: 500,
      tokens_out: 200,
      recorded_at: "2026-06-01T00:00:00Z",
    };
    mockInvoke.mockResolvedValueOnce(usage);
    const result = await useIntelligenceStore
      .getState()
      .recordTokenUsage("s1", null, "chat", "claude-3", 500, 200);
    expect(result).toEqual(usage);
  });

  it("getTokenUsageStats updates tokenStats", async () => {
    const stats = {
      total_tokens_in: 800,
      total_tokens_out: 350,
      total_tokens: 1150,
      by_model: [],
    };
    mockInvoke.mockResolvedValueOnce(stats);
    const result = await useIntelligenceStore.getState().getTokenUsageStats("s1");
    expect(result).toEqual(stats);
    expect(useIntelligenceStore.getState().tokenStats).toEqual(stats);
  });

  it("runHeartbeatCheck appends to heartbeatResults with cap", async () => {
    const hb = {
      session_id: "s1",
      state: "alive",
      last_activity_s: 5,
      checked_at: "2026-06-01T00:00:00Z",
    };
    mockInvoke.mockResolvedValueOnce(hb);
    await useIntelligenceStore
      .getState()
      .runHeartbeatCheck("s1", "2026-06-01T00:00:00Z", true);
    expect(useIntelligenceStore.getState().heartbeatResults).toEqual([hb]);
    expect(mockInvoke).toHaveBeenCalledWith("run_heartbeat_check_cmd", {
      sessionId: "s1",
      lastActivityAt: "2026-06-01T00:00:00Z",
      pidActive: true,
    });
  });
});
