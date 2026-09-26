import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface StackPreferences {
  preferred_stack: string;
  default_deploy_target: string;
  auto_provision: boolean;
}

export const DEFAULT_STACK_PREFERENCES: StackPreferences = {
  preferred_stack: "nextjs-supabase-vercel",
  default_deploy_target: "vercel",
  auto_provision: true,
};

interface SettingsState {
  theme: string;
  defaults: {
    projectPath: string;
    agentId: string;
    modelId: string;
  };
  stackPreferences: StackPreferences | null;
  onboardingCompleted: boolean;
  forceShowOnboarding: boolean;
  sidebarCollapsed: Record<string, boolean>;
  loadSettings: () => void;
  loadDefaults: () => Promise<void>;
  updateDefaults: (partial: { defaultStack?: string }) => Promise<void>;
  saveSettings: (partial: Partial<SettingsState>) => void;
  resetDefaults: () => void;
  setOnboardingCompleted: () => void;
  resetOnboarding: () => void;
  dismissOnboarding: () => void;
  isFirstLaunch: () => boolean;
  toggleSidebarGroup: (groupId: string) => void;
}

const STORAGE_KEY = "acc-settings";

const DEFAULT_SETTINGS = {
  theme: "dark",
  defaults: {
    projectPath: "",
    agentId: "opencode",
    modelId: "",
  },
  onboardingCompleted: false,
  forceShowOnboarding: false,
  sidebarCollapsed: {
    work: false,
    review: true,
    configure: true,
    automate: true,
    system: true,
  },
};

export const useSettingsStore = create<SettingsState>((set, get) => ({
  ...DEFAULT_SETTINGS,
  stackPreferences: null,

  loadSettings: () => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        set({
          theme: parsed.theme || DEFAULT_SETTINGS.theme,
          defaults: { ...DEFAULT_SETTINGS.defaults, ...parsed.defaults },
          onboardingCompleted: parsed.onboardingCompleted ?? false,
          sidebarCollapsed: { ...DEFAULT_SETTINGS.sidebarCollapsed, ...parsed.sidebarCollapsed },
        });
      }
    } catch {
      // Use defaults if parsing fails
    }
  },

  loadDefaults: async () => {
    try {
      const prefs = await invoke<StackPreferences>("get_preferences_cmd");
      set({ stackPreferences: prefs ?? DEFAULT_STACK_PREFERENCES });
    } catch {
      set({ stackPreferences: DEFAULT_STACK_PREFERENCES });
    }
  },

  updateDefaults: async (partial) => {
    const current = get().stackPreferences ?? DEFAULT_STACK_PREFERENCES;
    const next: StackPreferences = {
      ...current,
      preferred_stack: partial.defaultStack ?? current.preferred_stack,
    };
    try {
      const saved = await invoke<StackPreferences>("set_preferences_cmd", {
        preferences: next,
      });
      set({ stackPreferences: saved ?? next });
    } catch {
      set({ stackPreferences: next });
    }
  },

  saveSettings: (partial) => {    set(partial);
    const current = get();
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        theme: current.theme,
        defaults: current.defaults,
        onboardingCompleted: current.onboardingCompleted,
        sidebarCollapsed: current.sidebarCollapsed,
      })
    );
  },

  setOnboardingCompleted: () => {
    set({ onboardingCompleted: true });
    const current = get();
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        theme: current.theme,
        defaults: current.defaults,
        onboardingCompleted: true,
        sidebarCollapsed: current.sidebarCollapsed,
      })
    );
  },

  resetOnboarding: () => {
    set({ onboardingCompleted: false, forceShowOnboarding: true });
    const current = get();
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        theme: current.theme,
        defaults: current.defaults,
        onboardingCompleted: false,
        sidebarCollapsed: current.sidebarCollapsed,
      })
    );
  },

  dismissOnboarding: () => {
    set({ forceShowOnboarding: false });
  },

  isFirstLaunch: () => {
    const saved = localStorage.getItem(STORAGE_KEY);
    return !saved;
  },

  toggleSidebarGroup: (groupId) => {
    set((state) => ({
      sidebarCollapsed: {
        ...state.sidebarCollapsed,
        [groupId]: !state.sidebarCollapsed[groupId],
      },
    }));
  },

  resetDefaults: () => {
    set(DEFAULT_SETTINGS);
    localStorage.removeItem(STORAGE_KEY);
  },
}));
