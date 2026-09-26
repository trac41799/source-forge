import { useSettingsStore } from "@/stores/settingsStore";
import { mockInvoke } from "../setup";

beforeEach(() => {
  useSettingsStore.setState({
    theme: "dark",
    defaults: { projectPath: "", agentId: "opencode", modelId: "" },
    sidebarCollapsed: { work: false, review: true, configure: true, automate: true, system: true },
    stackPreferences: null,
  });
  localStorage.clear();
});

describe("settingsStore", () => {
  it("starts with dark theme and opencode agent", () => {
    const state = useSettingsStore.getState();
    expect(state.theme).toBe("dark");
    expect(state.defaults.agentId).toBe("opencode");
  });

  it("saveSettings persists to localStorage", () => {
    useSettingsStore
      .getState()
      .saveSettings({
        theme: "light",
        defaults: { projectPath: "/test", agentId: "claude", modelId: "claude-3" },
      });
    const saved = JSON.parse(localStorage.getItem("acc-settings")!);
    expect(saved.theme).toBe("light");
    expect(saved.defaults.projectPath).toBe("/test");
  });

  it("loadSettings reads from localStorage", () => {
    localStorage.setItem(
      "acc-settings",
      JSON.stringify({
        theme: "light",
        defaults: { projectPath: "/loaded", agentId: "windsurf", modelId: "gpt-4" },
      })
    );
    useSettingsStore.getState().loadSettings();
    const state = useSettingsStore.getState();
    expect(state.theme).toBe("light");
    expect(state.defaults.projectPath).toBe("/loaded");
  });

  it("loadSettings uses defaults when localStorage is corrupt", () => {
    localStorage.setItem("acc-settings", "not-json");
    useSettingsStore.getState().loadSettings();
    const state = useSettingsStore.getState();
    expect(state.theme).toBe("dark");
    expect(state.defaults.agentId).toBe("opencode");
  });

  it("resetDefaults clears localStorage and resets state", () => {
    useSettingsStore.setState({
      theme: "light",
      defaults: { projectPath: "/custom", agentId: "claude", modelId: "gpt-4" },
    });
    useSettingsStore.getState().resetDefaults();
    expect(useSettingsStore.getState().theme).toBe("dark");
    expect(useSettingsStore.getState().defaults.agentId).toBe("opencode");
    expect(localStorage.getItem("acc-settings")).toBeNull();
  });

  describe("sidebar collapse", () => {
    it("defaults: WORK open, all others collapsed", () => {
      const { sidebarCollapsed } = useSettingsStore.getState();
      expect(sidebarCollapsed.work).toBe(false);
      expect(sidebarCollapsed.review).toBe(true);
      expect(sidebarCollapsed.configure).toBe(true);
      expect(sidebarCollapsed.automate).toBe(true);
      expect(sidebarCollapsed.system).toBe(true);
    });

    it("toggleSidebarGroup flips collapse state", () => {
      const { toggleSidebarGroup } = useSettingsStore.getState();
      toggleSidebarGroup("review");
      expect(useSettingsStore.getState().sidebarCollapsed.review).toBe(false);
      toggleSidebarGroup("review");
      expect(useSettingsStore.getState().sidebarCollapsed.review).toBe(true);
    });

    it("persists collapse state to localStorage on save", () => {
      const { toggleSidebarGroup, saveSettings } = useSettingsStore.getState();
      toggleSidebarGroup("configure");
      saveSettings({});
      const saved = JSON.parse(localStorage.getItem("acc-settings")!);
      expect(saved.sidebarCollapsed.configure).toBe(false);
    });

    it("loads collapse state from localStorage on init", () => {
      localStorage.setItem("acc-settings", JSON.stringify({
        sidebarCollapsed: { work: true, review: false, configure: true, automate: true, system: true },
      }));
      useSettingsStore.getState().loadSettings();
      const { sidebarCollapsed } = useSettingsStore.getState();
      expect(sidebarCollapsed.work).toBe(true);
      expect(sidebarCollapsed.review).toBe(false);
    });
  });

  describe("stack preferences (backend)", () => {
    it("loadDefaults reads preferences from the backend", async () => {
      mockInvoke.mockResolvedValueOnce({
        preferred_stack: "express-react-supabase",
        default_deploy_target: "vercel",
        auto_provision: true,
      });

      await useSettingsStore.getState().loadDefaults();

      expect(mockInvoke).toHaveBeenCalledWith("get_preferences_cmd");
      expect(useSettingsStore.getState().stackPreferences?.preferred_stack).toBe(
        "express-react-supabase"
      );
    });

    it("updateDefaults persists defaultStack via set_preferences_cmd", async () => {
      mockInvoke
        .mockResolvedValueOnce({
          preferred_stack: "nextjs-supabase-vercel",
          default_deploy_target: "vercel",
          auto_provision: true,
        })
        .mockResolvedValueOnce({
          preferred_stack: "nextjs-supabase-fastapi",
          default_deploy_target: "vercel",
          auto_provision: true,
        });

      await useSettingsStore.getState().loadDefaults();
      await useSettingsStore.getState().updateDefaults({
        defaultStack: "nextjs-supabase-fastapi",
      });

      expect(mockInvoke).toHaveBeenLastCalledWith("set_preferences_cmd", {
        preferences: {
          preferred_stack: "nextjs-supabase-fastapi",
          default_deploy_target: "vercel",
          auto_provision: true,
        },
      });
      expect(useSettingsStore.getState().stackPreferences?.preferred_stack).toBe(
        "nextjs-supabase-fastapi"
      );
    });

    it("updateDefaults falls back to defaults when nothing was loaded", async () => {
      mockInvoke.mockResolvedValueOnce({
        preferred_stack: "nextjs-prisma-vercel",
        default_deploy_target: "vercel",
        auto_provision: true,
      });

      await useSettingsStore.getState().updateDefaults({
        defaultStack: "nextjs-prisma-vercel",
      });

      expect(mockInvoke).toHaveBeenCalledWith("set_preferences_cmd", {
        preferences: {
          preferred_stack: "nextjs-prisma-vercel",
          default_deploy_target: "vercel",
          auto_provision: true,
        },
      });
    });

    it("loadDefaults falls back to defaults when the backend fails", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no db"));

      await useSettingsStore.getState().loadDefaults();

      expect(useSettingsStore.getState().stackPreferences?.preferred_stack).toBe(
        "nextjs-supabase-vercel"
      );
    });
  });
});
