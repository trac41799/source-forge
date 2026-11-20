import { useAgentStore } from "@/stores/agentStore";
import { mockInvoke } from "../setup";

const sampleConfig = {
  id: "test-agent",
  label: "Test Agent",
  spawnCmd: "opencode",
  defaultArgs: ["--prompt", "{prompt}"],
  memoryFile: ".claude.md",
  globalConfigPath: "",
  mcpConfigFile: "mcp.json",
  mcpConfigKey: "test",
  tier: 1 as const,
  requiresAuth: undefined,
  supportsSubagents: true,
  subagentDetectionPattern: undefined,
  waveCommand: undefined,
  waveEligible: true,
  knownFlagVersions: undefined,
};

beforeEach(() => {
  useAgentStore.setState({ agents: new Map() });
});

describe("agentStore", () => {
  it("starts with empty agents map", () => {
    const { agents } = useAgentStore.getState();
    expect(agents.size).toBe(0);
  });

  it("spawns_with_valid_ipc_payload", async () => {
    mockInvoke.mockResolvedValueOnce({ installed: true, command: "opencode" });
    mockInvoke.mockResolvedValueOnce("session-123");
    await useAgentStore.getState().spawnAgent(sampleConfig, "/test/proj");
    const { agents } = useAgentStore.getState();
    expect(agents.size).toBe(1);
    const session = agents.get("test-agent");
    expect(session).toBeDefined();
    expect(session!.config.id).toBe("test-agent");
    expect(session!.status).toBe("idle");
    expect(session!.projectPath).toBe("/test/proj");
    expect(mockInvoke).toHaveBeenCalledWith("spawn_agent", {
      agentId: "test-agent",
      projectPath: "/test/proj",
      command: "opencode",
      args: ["--prompt", "{prompt}"],
      envVars: {},
    });
  });

  it("spawnAgent removes agent on invoke failure", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("spawn failed"));
    await expect(
      useAgentStore.getState().spawnAgent(sampleConfig, "/test/proj")
    ).rejects.toThrow("spawn failed");
    const { agents } = useAgentStore.getState();
    expect(agents.size).toBe(0);
  });

  it("killAgent removes agent and calls invoke", async () => {
    useAgentStore.setState({
      agents: new Map([
        [
          "test-agent",
          {
            config: sampleConfig,
            sessionId: "s1",
            status: "idle" as const,
            output: [],
            startedAt: new Date(),
            projectPath: "/p",
          },
        ],
      ]),
    });
    mockInvoke.mockResolvedValueOnce(undefined);
    await useAgentStore.getState().killAgent("test-agent");
    expect(useAgentStore.getState().agents.size).toBe(0);
    expect(mockInvoke).toHaveBeenCalledWith("kill_agent", {
      agentId: "test-agent",
    });
  });

  it("writeToAgent calls invoke with correct params", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await useAgentStore.getState().writeToAgent("test-agent", "hello");
    expect(mockInvoke).toHaveBeenCalledWith("write_to_agent", {
      agentId: "test-agent",
      text: "hello",
    });
  });

  it("updateStatus updates agent status in place", () => {
    useAgentStore.setState({
      agents: new Map([
        [
          "test-agent",
          {
            config: sampleConfig,
            sessionId: "s1",
            status: "idle" as const,
            output: [],
            startedAt: new Date(),
            projectPath: "/p",
          },
        ],
      ]),
    });
    useAgentStore.getState().updateStatus("test-agent", "thinking");
    expect(
      useAgentStore.getState().agents.get("test-agent")!.status
    ).toBe("thinking");
  });

  it("appendOutput adds text to agent output", () => {
    useAgentStore.setState({
      agents: new Map([
        [
          "test-agent",
          {
            config: sampleConfig,
            sessionId: "s1",
            status: "idle" as const,
            output: [],
            startedAt: new Date(),
            projectPath: "/p",
          },
        ],
      ]),
    });
    useAgentStore.getState().appendOutput("test-agent", "line1");
    useAgentStore.getState().appendOutput("test-agent", "line2");
    const output = useAgentStore.getState().agents.get("test-agent")!.output;
    expect(output).toEqual(["line1", "line2"]);
  });

  it("appendOutput caps at MAX_OUTPUT_LINES", () => {
    const initialOutput = Array.from({ length: 1000 }, (_, i) => `line${i}`);
    useAgentStore.setState({
      agents: new Map([
        [
          "test-agent",
          {
            config: sampleConfig,
            sessionId: "s1",
            status: "idle" as const,
            output: initialOutput,
            startedAt: new Date(),
            projectPath: "/p",
          },
        ],
      ]),
    });
    useAgentStore.getState().appendOutput("test-agent", "overflow");
    const output = useAgentStore.getState().agents.get("test-agent")!.output;
    expect(output.length).toBe(1000);
    expect(output[output.length - 1]).toBe("overflow");
    expect(output[0]).toBe("line1");
  });

  it("has streamOutput action", () => {
    const store = useAgentStore.getState();
    expect(store.streamOutput).toBeDefined();
    expect(typeof store.streamOutput).toBe("function");
  });

  it("streamOutput is callable", () => {
    const store = useAgentStore.getState();
    expect(() => store.streamOutput("test-agent")).not.toThrow();
  });
});
