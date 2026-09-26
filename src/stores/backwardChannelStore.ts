import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { IPC } from "@/lib/ipc/commands";
import type { ChatPlatformConfig, DaemonStatus, QueueInfo } from "@/lib/types";

interface BackwardChannelStore {
  platformConfigs: ChatPlatformConfig[];
  daemonStatus: DaemonStatus;
  queueInfo: QueueInfo;
  loading: boolean;
  error: string | null;

  getPlatformConfigs: (projectId: string) => Promise<void>;
  savePlatformConfig: (config: ChatPlatformConfig) => Promise<void>;
  deletePlatformConfig: (id: string) => Promise<void>;
  togglePlatformConfig: (id: string, enabled: boolean) => Promise<void>;

  startDaemon: (configPath: string) => Promise<void>;
  stopDaemon: () => Promise<void>;
  getDaemonStatus: () => Promise<void>;
  getDaemonLogs: (lines: number) => Promise<string[]>;

  checkQueueHealth: () => Promise<void>;
  testPlatformConnection: (platform: string, config: Record<string, string>) => Promise<boolean>;

  clearError: () => void;
}

export const useBackwardChannelStore = create<BackwardChannelStore>((set) => ({
  platformConfigs: [],
  daemonStatus: {
    running: false,
    pid: null,
    uptime_s: null,
    queue_depth: 0,
    active_platforms: [],
    last_event_at: null,
    error: null,
  },
  queueInfo: {
    provider: "upstash",
    connected: false,
    queue_depth: 0,
    latency_ms: null,
  },
  loading: false,
  error: null,

  getPlatformConfigs: async (projectId) => {
    set({ loading: true, error: null });
    try {
      const configs = await invoke<ChatPlatformConfig[]>(
        IPC.getChatPlatformConfigs,
        { projectId },
      );
      set({ platformConfigs: configs ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  savePlatformConfig: async (config) => {
    set({ loading: true, error: null });
    try {
      await invoke(IPC.saveChatPlatformConfig, { config });
      set({ loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  deletePlatformConfig: async (id) => {
    set({ loading: true, error: null });
    try {
      await invoke(IPC.deleteChatPlatformConfig, { id });
      set({ loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  togglePlatformConfig: async (id, enabled) => {
    set({ error: null });
    try {
      await invoke(IPC.toggleChatPlatformConfig, { id, enabled });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  startDaemon: async (configPath) => {
    set({ loading: true, error: null });
    try {
      await invoke(IPC.startBackwardChannelDaemon, { configPath });
      set({ loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  stopDaemon: async () => {
    set({ loading: true, error: null });
    try {
      await invoke(IPC.stopBackwardChannelDaemon);
      set({ loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  getDaemonStatus: async () => {
    set({ loading: true, error: null });
    try {
      const status = await invoke<DaemonStatus>(
        IPC.getBackwardChannelDaemonStatus,
      );
      set({ daemonStatus: status, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  getDaemonLogs: async (lines) => {
    set({ loading: true, error: null });
    try {
      const logs = await invoke<string[]>(
        IPC.getBackwardChannelDaemonLogs,
        { lines },
      );
      set({ loading: false });
      return logs ?? [];
    } catch (e) {
      set({ error: String(e), loading: false });
      return [];
    }
  },

  checkQueueHealth: async () => {
    set({ loading: true, error: null });
    try {
      const info = await invoke<QueueInfo>(
        IPC.checkBackwardChannelQueueHealth,
      );
      set({ queueInfo: info, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  testPlatformConnection: async (platform, config) => {
    set({ loading: true, error: null });
    try {
      const ok = await invoke<boolean>(
        IPC.testChatPlatformConnection,
        { platform, config },
      );
      set({ loading: false });
      return ok;
    } catch (e) {
      set({ error: String(e), loading: false });
      return false;
    }
  },

  clearError: () => set({ error: null }),
}));
