import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { IPC } from "@/lib/ipc/commands";

export interface TaskSuggestion {
  agent_id: string;
  model_id: string;
  confidence: number;
  reasoning: string;
  success_rate: number;
}

export interface ModelEntry {
  id: string;
  label: string;
  provider: string;
  model_path: string;
  strengths: string | null;
  agent_id: string | null;
  alternation_index: number | null;
  is_active: boolean;
}

export interface HandoffEnvelope {
  original_task: string;
  completed_by: string;
  model_used: string;
  output_summary: string;
  changed_files: string[];
  diff_preview: string;
  handoff_instruction: string;
  next_agent: string;
  next_model: string;
}

export interface WavePlan {
  id: string;
  project_id: string;
  slug: string;
  docs_path: string | null;
  status: string;
  created_at: string;
  completed_at: string | null;
}

export interface AgentExecution {
  agent_ref: string;
  session_id: string;
  worktree_path: string;
  branch: string;
  status: string;
  guideline_path: string;
  cost_usd: number;
}

export interface WaveExecutionReport {
  plan_id: string;
  base_repo: string;
  agents: AgentExecution[];
  started_at: string;
  completed_at: string | null;
  total_cost_usd: number;
}

/** Serde representation of Rust `CheckStatus` (unit + newtype variants). */
export type VerificationCheckStatus =
  | "Pass"
  | { Fail: string }
  | { Skip: string };

export interface VerificationCheckResult {
  name: string;
  status: VerificationCheckStatus;
  detail: string;
}

export interface VerificationReport {
  passed: boolean;
  checks: VerificationCheckResult[];
}

export interface VerifyAndFinalizeResult {
  wave: WaveExecutionReport;
  verification: VerificationReport;
}

export interface ParsedHandoffFile {
  original_task: string;
  completed_by: string;
  model_used: string;
  output_summary: string;
  changed_files: string[];
  handoff_instruction: string;
  raw_path: string;
}

export interface PlanAgent {
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

export interface CorrectionDoc {
  id: string;
  plan_id: string;
  agent_ref: string;
  bug_desc: string | null;
  root_cause: string | null;
  fix_required: string | null;
  test_required: string | null;
  retry_number: number;
  resolved: boolean;
  created_at: string;
}

export interface ACBSignal {
  id: string;
  session_id: string;
  wave: number | null;
  from_agent: string;
  to_agent: string;
  signal_type: string;
  priority: string;
  body: string;
  ref_id: string | null;
  status: string;
  created_at: string;
  resolved_at: string | null;
}

export interface PlaybookManifest {
  version: string;
  name: string;
  project: string;
  exported_at: string;
  stacks: string[];
  includes: string[];
}

export interface MemoryCandidate {
  id: string;
  session_id: string;
  project_id: string;
  content: string;
  source_pattern: string | null;
  status: string;
  created_at: string;
}

interface OrchestrationStore {
  // Routing
  suggestions: TaskSuggestion[];
  models: ModelEntry[];
  // Orchestrator
  wavePlans: WavePlan[];
  planAgents: PlanAgent[];
  corrections: CorrectionDoc[];
  // ACB
  acbSignals: ACBSignal[];
  // Playbook
  memoryCandidates: MemoryCandidate[];
  playbookManifest: PlaybookManifest | null;

  // Routing actions
  routeTask: (taskDesc: string, taskType: string, projectId?: string) => Promise<void>;
  getModels: () => Promise<void>;
  addModel: (entry: ModelEntry) => Promise<void>;
  toggleModel: (modelId: string, active: boolean) => Promise<void>;
  buildHandoff: (brief: HandoffEnvelope) => Promise<string>;
  checkAgentVersion: (agentId: string) => Promise<void>;
  // Orchestrator actions
  createWavePlan: (projectId: string, slug: string) => Promise<WavePlan>;
  addPlanAgent: (planId: string, agentRef: string, task: string, wave: number, dependsOn?: string, agentId?: string) => Promise<void>;
  getPlanAgents: (planId: string) => Promise<void>;
  updatePlanAgentStatus: (agentId: string, status: string) => Promise<void>;
  generateGuideline: (agentRef: string, task: string, objective: string, dependsOn?: string, models?: string[], filesToCreate?: string[], filesNotTouch?: string[]) => Promise<string>;
  validateHandoff: (content: string) => Promise<{ valid: boolean; missing: string[] }>;
  executeWave: (
    planId: string,
    baseRepo: string,
    baseBranch: string,
    agentCommand: string,
    agentBaseArgs: string[],
    deadlineSecs?: number,
    costCapUsd?: number,
  ) => Promise<WaveExecutionReport>;
  finalizeWave: (
    report: WaveExecutionReport,
    projectPath: string
  ) => Promise<VerifyAndFinalizeResult>;
  createWorktree: (repoPath: string, branch: string, worktreePath: string, baseBranch: string) => Promise<string>;
  removeWorktree: (repoPath: string, worktreePath: string) => Promise<void>;
  listWorktrees: (repoPath: string) => Promise<string[]>;
  parseHandoffFile: (path: string) => Promise<ParsedHandoffFile>;
  createCorrection: (planId: string, agentRef: string, bugDesc: string, rootCause: string, fixRequired: string, testRequired: string, retryNumber: number) => Promise<void>;
  getCorrections: (planId: string) => Promise<void>;
  // ACB actions
  parseAcbSignal: (line: string) => Promise<ACBSignal | null>;
  recordAcbSignal: (signal: ACBSignal) => Promise<void>;
  getOpenSignals: (sessionId?: string) => Promise<void>;
  resolveSignal: (signalId: string) => Promise<void>;
  // Playbook actions
  detectMemoryCandidate: (output: string) => Promise<string | null>;
  createMemoryCandidate: (sessionId: string, projectId: string, content: string, sourcePattern?: string) => Promise<void>;
  getMemoryCandidates: (sessionId?: string, status?: string) => Promise<void>;
  buildPlaybookManifest: (name: string, project: string, stacks: string[], includeSkills: boolean, includeMemory: boolean, includePresets: boolean) => Promise<void>;
  buildFeatureDocPrompt: (docType: string, sessionId: string, featureName: string) => Promise<string>;
}

export const useOrchestrationStore = create<OrchestrationStore>((set) => ({
  suggestions: [],
  models: [],
  wavePlans: [],
  planAgents: [],
  corrections: [],
  acbSignals: [],
  memoryCandidates: [],
  playbookManifest: null,

  routeTask: async (taskDesc, taskType, projectId) => {
    const suggestions = await invoke<TaskSuggestion[]>("route_task_cmd", { taskDesc, taskType, projectId });
    set({ suggestions });
  },
  getModels: async () => {
    const models = await invoke<ModelEntry[]>("get_models_cmd");
    set({ models });
  },
  addModel: async (entry) => { await invoke("add_model_cmd", { entry }); },
  toggleModel: async (modelId, active) => { await invoke("toggle_model_cmd", { modelId, active }); },
  buildHandoff: async (brief) => {
    return await invoke<string>("build_handoff_cmd", { agentBrief: brief });
  },
  checkAgentVersion: async (agentId) => {
    await invoke("check_agent_version_cmd", { agentId });
  },
  createWavePlan: async (projectId, slug) => {
    const plan = await invoke<WavePlan>("create_wave_plan_cmd", { projectId, slug });
    set((s) => ({ wavePlans: [...s.wavePlans, plan] }));
    return plan;
  },
  addPlanAgent: async (planId, agentRef, task, wave, dependsOn, agentId) => {
    await invoke("add_plan_agent_cmd", { planId, agentRef, task, wave, dependsOn, agentId });
  },
  getPlanAgents: async (planId) => {
    const agents = await invoke<PlanAgent[]>("get_plan_agents_cmd", { planId });
    set({ planAgents: agents });
  },
  updatePlanAgentStatus: async (agentId, status) => {
    await invoke("update_plan_agent_status_cmd", { agentId, status });
  },
  generateGuideline: async (agentRef, task, objective, dependsOn, models = [], filesToCreate = [], filesNotTouch = []) => {
    return await invoke<string>("generate_guideline_cmd", { agentRef, task, objective, dependsOn, models, filesToCreate, filesNotTouch });
  },
  validateHandoff: async (content) => {
    const [valid, missing] = await invoke<[boolean, string[]]>("validate_handoff_schema_cmd", { content });
    return { valid, missing };
  },
  executeWave: async (planId, baseRepo, baseBranch, agentCommand, agentBaseArgs, deadlineSecs?, costCapUsd?) => {
    return await invoke<WaveExecutionReport>(IPC.executeWave, {
      planId,
      baseRepo,
      baseBranch,
      agentCommand,
      agentBaseArgs,
      deadlineSecs: deadlineSecs ?? null,
      costCapUsd: costCapUsd ?? null,
    });
  },
  finalizeWave: async (report, projectPath) => {
    // Verification gate (SPEC-001 §5 DG-4): the UI never finalizes on handoff
    // file existence alone — every finalize runs deployment verification and
    // returns both the wave report and the verification report.
    return await invoke<VerifyAndFinalizeResult>(IPC.verifyAndFinalizeWave, {
      report,
      projectPath,
    });
  },
  createWorktree: async (repoPath, branch, worktreePath, baseBranch) => {
    return await invoke<string>("create_worktree_cmd", { repoPath, branch, worktreePath, baseBranch });
  },
  removeWorktree: async (repoPath, worktreePath) => {
    return await invoke<void>("remove_worktree_cmd", { repoPath, worktreePath });
  },
  listWorktrees: async (repoPath) => {
    return await invoke<string[]>("list_worktrees_cmd", { repoPath });
  },
  parseHandoffFile: async (path) => {
    return await invoke<ParsedHandoffFile>("parse_handoff_file_cmd", { path });
  },
  createCorrection: async (planId, agentRef, bugDesc, rootCause, fixRequired, testRequired, retryNumber) => {
    await invoke("create_correction_cmd", { planId, agentRef, bugDesc, rootCause, fixRequired, testRequired, retryNumber });
  },
  getCorrections: async (planId) => {
    const corrections = await invoke<CorrectionDoc[]>("get_corrections_cmd", { planId });
    set({ corrections });
  },
  parseAcbSignal: async (line) => {
    return await invoke<ACBSignal | null>("parse_acb_signal_cmd", { line });
  },
  recordAcbSignal: async (signal) => {
    await invoke("record_acb_signal_cmd", { signal });
    set((s) => ({ acbSignals: [...s.acbSignals, signal] }));
  },
  getOpenSignals: async (sessionId) => {
    const signals = await invoke<ACBSignal[]>("get_open_signals_cmd", { sessionId });
    set({ acbSignals: signals ?? [] });
  },
  resolveSignal: async (signalId) => {
    await invoke("resolve_signal_cmd", { signalId });
    set((s) => ({ acbSignals: s.acbSignals.filter((sig) => sig.id !== signalId) }));
  },
  detectMemoryCandidate: async (output) => {
    return await invoke<string | null>("detect_memory_candidate_cmd", { output });
  },
  createMemoryCandidate: async (sessionId, projectId, content, sourcePattern) => {
    await invoke("create_memory_candidate_cmd", { sessionId, projectId, content, sourcePattern });
  },
  getMemoryCandidates: async (sessionId, status) => {
    const candidates = await invoke<MemoryCandidate[]>("get_memory_candidates_cmd", { sessionId, status });
    set({ memoryCandidates: candidates ?? [] });
  },
  buildPlaybookManifest: async (name, project, stacks, includeSkills, includeMemory, includePresets) => {
    const manifest = await invoke<PlaybookManifest>("build_playbook_manifest_cmd", { name, project, stacks, includeSkills, includeMemory, includePresets });
    set({ playbookManifest: manifest });
  },
  buildFeatureDocPrompt: async (docType, sessionId, featureName) => {
    return await invoke<string>("build_feature_doc_prompt_cmd", { docType, sessionId, featureName });
  },
}));
