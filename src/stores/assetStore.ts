import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { McpInstallStatus, McpConnectionTest } from "@/lib/types";

export interface SkillEntry {
  id: string;
  name: string;
  path: string;
  source: string;
  content: string;
  tags: string[];
  injectable: boolean;
}

export interface MemoryFileEntry {
  id: string;
  name: string;
  path: string;
  agent: string;
  content: string;
  last_modified: string;
  snapshot: string | null;
}

export interface MCPEntry {
  id: string;
  name: string;
  server_command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
  source: string;
  agent_id: string;
  managed_externally: boolean;
  health: string;
}

export interface VaultEntry {
  id: string;
  key_name: string;
  scope: string;
  agent_id: string | null;
  project_id: string | null;
  masked_value: string;
  created_at: string;
}

interface AssetStore {
  skills: SkillEntry[];
  memoryFiles: MemoryFileEntry[];
  mcps: MCPEntry[];
  secrets: VaultEntry[];
  plugins: string[];
  projectProfile: Record<string, unknown> | null;
  selectedSkill: SkillEntry | null;
  selectedMemoryFile: MemoryFileEntry | null;
  loading: boolean;
  error: string | null;
  baguaMcpStatus: McpInstallStatus | null;
  baguaMcpConnection: McpConnectionTest | null;

  scanSkills: (path: string) => Promise<void>;
  readSkill: (path: string) => Promise<string>;
  scanMemory: (projectPath: string) => Promise<void>;
  readMemoryFile: (path: string) => Promise<void>;
  writeMemoryFile: (path: string, content: string) => Promise<void>;
  listMcps: (agentConfigPath: string) => Promise<void>;
  toggleMcp: (
    agentConfigPath: string,
    mcpName: string,
    enabled: boolean
  ) => Promise<void>;
  storeSecret: (
    keyName: string,
    value: string,
    scope: string,
    agentId?: string,
    projectId?: string
  ) => Promise<void>;
  deleteSecret: (id: string) => Promise<void>;
  listSecrets: () => Promise<void>;
  getSecretValue: (id: string) => Promise<string>;
  listPlugins: () => Promise<void>;
  generateProfile: (projectPath: string) => Promise<void>;
  setSelectedSkill: (skill: SkillEntry | null) => void;
  setSelectedMemoryFile: (file: MemoryFileEntry | null) => void;
  clearError: () => void;
  detectBaguaMcp: () => Promise<void>;
  testBaguaMcpConnection: () => Promise<void>;
  getBaguaMcpConfig: () => MCPEntry;
}

export const useAssetStore = create<AssetStore>((set) => ({
  skills: [],
  memoryFiles: [],
  mcps: [],
  secrets: [],
  plugins: [],
  projectProfile: null,
  selectedSkill: null,
  selectedMemoryFile: null,
  loading: false,
  error: null,
  baguaMcpStatus: null,
  baguaMcpConnection: null,

  scanSkills: async (path: string) => {
    set({ loading: true, error: null });
    try {
      const skills = await invoke<SkillEntry[]>("scan_skills", { path });
      set({ skills: skills ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  readSkill: async (path: string) => {
    return await invoke<string>("read_skill", { path });
  },

  scanMemory: async (projectPath: string) => {
    set({ loading: true, error: null });
    try {
      const memoryFiles = await invoke<MemoryFileEntry[]>("scan_memory", {
        projectPath,
      });
      set({ memoryFiles: memoryFiles ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  readMemoryFile: async (path: string) => {
    await invoke<string>("read_skill", { path });
    set((state) => ({
      selectedMemoryFile: state.memoryFiles.find((f) => f.path === path) || null,
    }));
  },

  writeMemoryFile: async (path: string, content: string) => {
    await invoke("write_memory", { path, content });
  },

  listMcps: async (agentConfigPath: string) => {
    set({ loading: true, error: null });
    try {
      const mcps = await invoke<MCPEntry[]>("list_mcps", { agentConfigPath });
      set({ mcps: mcps ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  toggleMcp: async (
    agentConfigPath: string,
    mcpName: string,
    enabled: boolean
  ) => {
    await invoke("toggle_mcp", { agentConfigPath, mcpName, enabled });
    set((state) => ({
      mcps: state.mcps.map((m) =>
        m.name === mcpName
          ? { ...m, enabled, health: enabled ? "green" : "grey" }
          : m
      ),
    }));
  },

  storeSecret: async (
    keyName: string,
    value: string,
    scope: string,
    agentId?: string,
    projectId?: string
  ) => {
    await invoke("store_secret", { keyName, value, scope, agentId, projectId });
    const secrets = await invoke<VaultEntry[]>("list_secrets");
    set({ secrets: secrets ?? [] });
  },

  deleteSecret: async (id: string) => {
    set((state) => ({
      secrets: state.secrets.filter((s) => s.id !== id),
    }));
  },

  listSecrets: async () => {
    set({ loading: true, error: null });
    try {
      const secrets = await invoke<VaultEntry[]>("list_secrets");
      set({ secrets: secrets ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  getSecretValue: async (id: string) => {
    return await invoke<string>("get_secret_value", { id });
  },

  listPlugins: async () => {
    set({ loading: true, error: null });
    try {
      const plugins = await invoke<string[]>("list_plugins");
      set({ plugins: plugins ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  generateProfile: async (projectPath: string) => {
    set({ loading: true, error: null });
    try {
      const projectProfile = await invoke<Record<string, unknown>>(
        "generate_profile",
        { projectPath }
      );
      set({ projectProfile, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  setSelectedSkill: (skill) => set({ selectedSkill: skill }),
  setSelectedMemoryFile: (file) => set({ selectedMemoryFile: file }),
  clearError: () => set({ error: null }),

  detectBaguaMcp: async () => {
    try {
      const status = await invoke<McpInstallStatus>("detect_bagua_mcp_cmd");
      set({ baguaMcpStatus: status });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  testBaguaMcpConnection: async () => {
    set({ loading: true, error: null });
    try {
      const result = await invoke<McpConnectionTest>("test_bagua_mcp_connection_cmd");
      set({ baguaMcpConnection: result, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  getBaguaMcpConfig: () => ({
    id: "mcp-builtin-bagua-semantic-kg",
    name: "GA-Bagua Semantic KG",
    server_command: "ga-semantics-mcp",
    args: [],
    env: {},
    enabled: false,
    source: "builtin",
    agent_id: "system",
    managed_externally: false,
    health: "grey",
  }),
}));
