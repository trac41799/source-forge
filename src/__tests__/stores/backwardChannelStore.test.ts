import { useBackwardChannelStore } from "@/stores/backwardChannelStore";
import type { ChatPlatformConfig } from "@/lib/types";
import { mockInvoke } from "../setup";

beforeEach(() => {
  useBackwardChannelStore.setState({
    platformConfigs: [],
    loading: false,
    error: null,
  });
});

const config = { id: "cfg-1", platform: "slack" } as ChatPlatformConfig;

describe("backwardChannelStore IPC names (SPEC-001 §3.2)", () => {
  it("getPlatformConfigs → get_chat_platform_configs_cmd", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    await useBackwardChannelStore.getState().getPlatformConfigs("proj-1");
    expect(mockInvoke).toHaveBeenCalledWith("get_chat_platform_configs_cmd", {
      projectId: "proj-1",
    });
  });

  it("savePlatformConfig → save_chat_platform_config_cmd", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await useBackwardChannelStore.getState().savePlatformConfig(config);
    expect(mockInvoke).toHaveBeenCalledWith("save_chat_platform_config_cmd", {
      config,
    });
  });

  it("deletePlatformConfig → delete_chat_platform_config_cmd", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await useBackwardChannelStore.getState().deletePlatformConfig("cfg-1");
    expect(mockInvoke).toHaveBeenCalledWith("delete_chat_platform_config_cmd", {
      id: "cfg-1",
    });
  });

  it("togglePlatformConfig → toggle_chat_platform_config_cmd", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await useBackwardChannelStore
      .getState()
      .togglePlatformConfig("cfg-1", true);
    expect(mockInvoke).toHaveBeenCalledWith("toggle_chat_platform_config_cmd", {
      id: "cfg-1",
      enabled: true,
    });
  });

  it("startDaemon → start_backward_channel_daemon_cmd", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await useBackwardChannelStore.getState().startDaemon("/tmp/config.yml");
    expect(mockInvoke).toHaveBeenCalledWith(
      "start_backward_channel_daemon_cmd",
      { configPath: "/tmp/config.yml" }
    );
  });

  it("stopDaemon → stop_backward_channel_daemon_cmd", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await useBackwardChannelStore.getState().stopDaemon();
    expect(mockInvoke).toHaveBeenCalledWith(
      "stop_backward_channel_daemon_cmd"
    );
  });

  it("getDaemonStatus → get_backward_channel_daemon_status_cmd", async () => {
    mockInvoke.mockResolvedValueOnce({
      running: false,
      pid: null,
      uptime_s: null,
      queue_depth: 0,
      active_platforms: [],
      last_event_at: null,
      error: null,
    });
    await useBackwardChannelStore.getState().getDaemonStatus();
    expect(mockInvoke).toHaveBeenCalledWith(
      "get_backward_channel_daemon_status_cmd"
    );
  });

  it("getDaemonLogs → get_backward_channel_daemon_logs_cmd", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    await useBackwardChannelStore.getState().getDaemonLogs(50);
    expect(mockInvoke).toHaveBeenCalledWith(
      "get_backward_channel_daemon_logs_cmd",
      { lines: 50 }
    );
  });

  it("checkQueueHealth → check_backward_channel_queue_health_cmd", async () => {
    mockInvoke.mockResolvedValueOnce({
      provider: "upstash",
      connected: true,
      queue_depth: 0,
      latency_ms: 12,
    });
    await useBackwardChannelStore.getState().checkQueueHealth();
    expect(mockInvoke).toHaveBeenCalledWith(
      "check_backward_channel_queue_health_cmd"
    );
  });

  it("testPlatformConnection → test_chat_platform_connection_cmd", async () => {
    mockInvoke.mockResolvedValueOnce(true);
    const ok = await useBackwardChannelStore
      .getState()
      .testPlatformConnection("slack", { bot_token: "x" });
    expect(mockInvoke).toHaveBeenCalledWith(
      "test_chat_platform_connection_cmd",
      { platform: "slack", config: { bot_token: "x" } }
    );
    expect(ok).toBe(true);
  });
});
