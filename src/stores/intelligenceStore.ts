import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { IPC } from "../lib/ipc/commands";

export interface OutcomeRecord {
  id: string;
  session_id: string;
  agent_id: string;
  task_type: string;
  outcome: string;
  duration_s: number;
  created_at: string;
}

export interface OutcomeStats {
  agent_id: string;
  task_type: string;
  project_id: string | null;
  total: number;
  done: number;
  failed: number;
  revised: number;
  avg_duration_s: number | null;
  success_rate: number;
}

export interface FailureAnalysis {
  id: string;
  session_id: string;
  pty_excerpt: string;
  diagnosis: string | null;
  root_cause: string | null;
  suggested_fix: string | null;
  confidence: number;
  created_at: string;
}

export interface LimitEvent {
  id: string;
  session_id: string;
  plan_agent_id: string | null;
  event_type: string;
  raw_message: string;
  resolved: boolean;
  resolved_at: string | null;
  resolution: string | null;
}

export interface TokenUsage {
  id: string;
  session_id: string;
  agent_id: string | null;
  context: string;
  model: string | null;
  tokens_in: number;
  tokens_out: number;
  recorded_at: string;
}

export interface TokenStats {
  total_tokens_in: number;
  total_tokens_out: number;
  total_tokens: number;
  by_model: Array<{ model: string | null; tokens_in: number; tokens_out: number; calls: number }>;
}

export interface HeartbeatResult {
  session_id: string;
  state: string;
  last_activity_s: number;
  checked_at: string;
}

interface IntelligenceStore {
  outcomeStats: OutcomeStats[];
  failureAnalyses: FailureAnalysis[];
  limitEvents: LimitEvent[];
  tokenStats: TokenStats | null;
  heartbeatResults: HeartbeatResult[];

  recordOutcome: (sessionId: string, agentId: string, taskType: string, outcome: string, durationS: number) => Promise<OutcomeRecord>;
  inferOutcome: (sessionId: string, agentId: string, taskType: string, ptyOutput: string, idleSeconds: number, durationS: number) => Promise<OutcomeRecord>;
  getOutcomeStats: (projectId?: string, agentId?: string) => Promise<void>;
  createFailureAnalysis: (sessionId: string, ptyExcerpt: string) => Promise<FailureAnalysis>;
  diagnoseFailure: (analysisId: string, diagnosis: string, rootCause: string, suggestedFix: string) => Promise<number>;
  getFailureAnalyses: (sessionId?: string, limit?: number) => Promise<void>;
  detectLimitEvent: (rawOutput: string) => Promise<[string, string] | null>;
  recordLimitEvent: (sessionId: string, planAgentId: string | null, eventType: string, rawMessage: string) => Promise<LimitEvent>;
  resolveLimitEvent: (eventId: string, resolution: string) => Promise<void>;
  getUnresolvedLimits: (sessionId?: string) => Promise<void>;
  recordTokenUsage: (sessionId: string, agentId: string | null, context: string, model: string | null, tokensIn: number, tokensOut: number) => Promise<TokenUsage>;
  getTokenUsageStats: (sessionId?: string) => Promise<TokenStats>;
  runHeartbeatCheck: (sessionId: string, lastActivityAt: string, pidActive: boolean) => Promise<void>;
}

export const useIntelligenceStore = create<IntelligenceStore>((set) => ({
  outcomeStats: [],
  failureAnalyses: [],
  limitEvents: [],
  tokenStats: null,
  heartbeatResults: [],

  recordOutcome: async (sessionId, agentId, taskType, outcome, durationS) => {
    const result = await invoke<OutcomeRecord>(IPC.recordOutcome, {
      sessionId,
      agentId,
      taskType,
      outcome,
      durationS,
    });
    return result;
  },

  inferOutcome: async (sessionId, agentId, taskType, ptyOutput, idleSeconds, durationS) => {
    return await invoke<OutcomeRecord>(IPC.inferOutcome, {
      sessionId,
      agentId,
      taskType,
      ptyOutput,
      idleSeconds,
      durationS,
    });
  },

  getOutcomeStats: async (projectId, agentId) => {
    const stats = await invoke<OutcomeStats[]>(IPC.getOutcomeStats, { projectId, agentId });
    set({ outcomeStats: stats ?? [] });
  },

  createFailureAnalysis: async (sessionId, ptyExcerpt) => {
    const result = await invoke<FailureAnalysis>(IPC.createFailureAnalysis, { sessionId, ptyExcerpt });
    set((state) => ({ failureAnalyses: [result, ...state.failureAnalyses] }));
    return result;
  },

  diagnoseFailure: async (analysisId, diagnosis, rootCause, suggestedFix) => {
    return await invoke<number>(IPC.diagnoseFailure, {
      analysisId,
      diagnosis,
      rootCause,
      suggestedFix,
    });
  },

  getFailureAnalyses: async (sessionId, limit) => {
    const analyses = await invoke<FailureAnalysis[]>(IPC.getFailureAnalyses, { sessionId, limit });
    set({ failureAnalyses: analyses });
  },

  detectLimitEvent: async (rawOutput) => {
    return await invoke<[string, string] | null>(IPC.detectLimitEvent, { rawOutput });
  },

  recordLimitEvent: async (sessionId, planAgentId, eventType, rawMessage) => {
    const result = await invoke<LimitEvent>(IPC.recordLimitEvent, {
      sessionId,
      planAgentId,
      eventType,
      rawMessage,
    });
    set((state) => ({ limitEvents: [...state.limitEvents, result] }));
    return result;
  },

  resolveLimitEvent: async (eventId, resolution) => {
    await invoke(IPC.resolveLimitEvent, { eventId, resolution });
    set((state) => ({
      limitEvents: state.limitEvents.map((e) =>
        e.id === eventId ? { ...e, resolved: true, resolution } : e
      ),
    }));
  },

  getUnresolvedLimits: async (sessionId) => {
    const events = await invoke<LimitEvent[]>(IPC.getUnresolvedLimits, { sessionId });
    set({ limitEvents: events });
  },

  recordTokenUsage: async (sessionId, agentId, context, model, tokensIn, tokensOut) => {
    const result = await invoke<TokenUsage>(IPC.recordTokenUsage, {
      sessionId,
      agentId,
      context,
      model,
      tokensIn,
      tokensOut,
    });
    return result;
  },

  getTokenUsageStats: async (sessionId) => {
    const stats = await invoke<TokenStats>(IPC.getTokenUsageStats, { sessionId });
    set({ tokenStats: stats });
    return stats;
  },

  runHeartbeatCheck: async (sessionId, lastActivityAt, pidActive) => {
    const result = await invoke<HeartbeatResult>(IPC.runHeartbeatCheck, {
      sessionId,
      lastActivityAt,
      pidActive,
    });
    set((state) => ({ heartbeatResults: [...state.heartbeatResults.slice(-50), result] }));
  },
}));
