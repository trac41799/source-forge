import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface CronJob {
  id: string;
  name: string;
  description?: string;
  project_id?: string;
  schedule: string;
  task_template: string;
  wave_preset?: string;
  auto_approve: boolean;
  escalation_policy: string;
  notification_channels?: string;
  max_correction_retries: number;
  enabled: boolean;
  last_run_at?: string;
  next_run_at?: string;
  created_at: string;
  updated_at: string;
}

export interface CronJobInput {
  name: string;
  description?: string;
  project_id?: string;
  schedule: string;
  task_template: string;
  wave_preset?: string;
  auto_approve?: boolean;
  escalation_policy?: string;
  notification_channels?: string;
  max_correction_retries?: number;
  enabled?: boolean;
}

export interface CronExecution {
  id: string;
  cron_job_id: string;
  plan_id?: string;
  status: string;
  escalation_reason?: string;
  escalation_source?: string;
  started_at: string;
  completed_at?: string;
  notified_at?: string;
}

export interface CronFiredEvent {
  job_id: string;
  plan_id: string;
  execution_id: string;
  timestamp: string;
}

export interface CronEscalatedEvent {
  job_id: string;
  execution_id: string;
  error: string;
  failure_count: number;
  timestamp: string;
}

interface SchedulerStore {
  jobs: CronJob[];
  executions: CronExecution[];
  nextRuns: string[];
  escalations: CronJob[];
  lastFired: CronFiredEvent | null;
  lastEscalation: CronEscalatedEvent | null;
  eventListenersAttached: boolean;
  loading: boolean;
  error: string | null;

  loadJobs: (projectId?: string) => Promise<void>;
  createJob: (input: CronJobInput) => Promise<CronJob>;
  updateJob: (id: string, data: CronJobInput) => Promise<CronJob>;
  deleteJob: (id: string) => Promise<void>;
  toggleJob: (id: string, enabled: boolean) => Promise<void>;
  runNow: (id: string) => Promise<CronExecution | null>;
  getExecutions: (jobId?: string) => Promise<void>;
  evaluateSchedule: (cronExpr: string, count?: number) => Promise<void>;
  loadEscalations: () => Promise<void>;
  clearEscalation: (id: string) => Promise<void>;
  attachEventListeners: () => Promise<UnlistenFn[]>;
  detachEventListeners: (unlistens: UnlistenFn[]) => void;
  pushExecution: (execution: CronExecution) => void;
  setLastFired: (event: CronFiredEvent | null) => void;
  setLastEscalation: (event: CronEscalatedEvent | null) => void;
  clearError: () => void;
}

export const useSchedulerStore = create<SchedulerStore>((set, get) => ({
  jobs: [],
  executions: [],
  nextRuns: [],
  escalations: [],
  lastFired: null,
  lastEscalation: null,
  eventListenersAttached: false,
  loading: false,
  error: null,

  loadJobs: async (projectId) => {
    set({ loading: true, error: null });
    try {
      const jobs = await invoke<CronJob[]>("get_cron_jobs_cmd", { projectId, enabledOnly: false });
      set({ jobs: jobs ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  createJob: async (input) => {
    set({ error: null });
    try {
      const job = await invoke<CronJob>("create_cron_job_cmd", { input });
      set((state) => ({ jobs: [...state.jobs, job] }));
      return job;
    } catch (e: unknown) {
      set({ error: String(e) });
      throw e;
    }
  },

  updateJob: async (id, data) => {
    set({ error: null });
    try {
      const updated = await invoke<CronJob>("update_cron_job_cmd", { id, input: data });
      set((state) => ({
        jobs: state.jobs.map((j) => (j.id === id ? updated : j)),
      }));
      return updated;
    } catch (e: unknown) {
      set({ error: String(e) });
      throw e;
    }
  },

  deleteJob: async (id) => {
    set({ error: null });
    try {
      await invoke("delete_cron_job_cmd", { id });
      set((state) => ({
        jobs: state.jobs.filter((j) => j.id !== id),
        escalations: state.escalations.filter((j) => j.id !== id),
      }));
    } catch (e: unknown) {
      set({ error: String(e) });
      throw e;
    }
  },

  toggleJob: async (id, enabled) => {
    set({ error: null });
    try {
      await invoke("toggle_cron_job_cmd", { id, enabled });
      set((state) => ({
        jobs: state.jobs.map((j) => (j.id === id ? { ...j, enabled } : j)),
      }));
    } catch (e: unknown) {
      set({ error: String(e) });
      throw e;
    }
  },

  runNow: async (id) => {
    set({ error: null });
    try {
      const execution = await invoke<CronExecution>("record_cron_execution_cmd", {
        jobId: id,
        planId: null,
      });
      set((state) => ({ executions: [execution, ...state.executions] }));
      return execution;
    } catch (e: unknown) {
      set({ error: String(e) });
      throw e;
    }
  },

  getExecutions: async (jobId) => {
    set({ loading: true, error: null });
    try {
      const executions = await invoke<CronExecution[]>("get_cron_executions_cmd", {
        jobId: jobId ?? null,
        status: null,
      });
      set({ executions: executions ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  evaluateSchedule: async (cronExpr, count = 5) => {
    set({ loading: true, error: null });
    try {
      const runs = await invoke<string[]>("evaluate_cron_schedule_cmd", {
        cronExpr,
        count,
      });
      set({ nextRuns: runs, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  loadEscalations: async () => {
    set({ error: null });
    try {
      const all = await invoke<CronJob[]>("get_cron_jobs_cmd", {
        projectId: null,
        enabledOnly: false,
      });
      set({ escalations: (all ?? []).filter((j) => j.enabled) });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  clearEscalation: async (id) => {
    set((state) => ({ escalations: state.escalations.filter((j) => j.id !== id) }));
  },

  attachEventListeners: async () => {
    if (get().eventListenersAttached) return [];
    const unlistens: UnlistenFn[] = [];

    const firedUn = await listen<CronFiredEvent>("cron-fired", (event) => {
      const payload = event.payload;
      useSchedulerStore.setState({ lastFired: payload });
      useSchedulerStore.getState().getExecutions(payload.job_id);
    });
    unlistens.push(firedUn);

    const escalatedUn = await listen<CronEscalatedEvent>(
      "cron-escalated",
      (event) => {
        const payload = event.payload;
        useSchedulerStore.setState({ lastEscalation: payload });
        useSchedulerStore.getState().loadEscalations();
      },
    );
    unlistens.push(escalatedUn);

    set({ eventListenersAttached: true });
    return unlistens;
  },

  detachEventListeners: (unlistens) => {
    for (const un of unlistens) {
      try { un(); } catch { console.warn('unlisten error'); }
    }
    set({ eventListenersAttached: false });
  },

  pushExecution: (execution) => {
    set((state) => ({ executions: [execution, ...state.executions] }));
  },

  setLastFired: (event) => set({ lastFired: event }),
  setLastEscalation: (event) => set({ lastEscalation: event }),

  clearError: () => set({ error: null }),
}));
