import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import Orchestrate from "@/pages/Orchestrate";
import { mockInvoke } from "@/__tests__/setup";
import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { useProjectStore } from "@/stores/projectStore";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

beforeEach(() => {
  useProjectStore.setState({
    currentProject: null,
    recentProjects: [],
    recentPaths: [],
  });
  useOrchestrationStore.setState({
    suggestions: [],
    models: [],
    wavePlans: [],
    planAgents: [],
    corrections: [],
    acbSignals: [],
    memoryCandidates: [],
    playbookManifest: null,
    lastWaveReport: null,
    lastVerification: null,
  });
});

function renderOrchestrate(initialEntries = ["/orchestrate"]) {
  mockInvoke.mockResolvedValue([]);
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <ThemeProvider>
        <Orchestrate />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Orchestrate tabs", () => {
  it("renders three tabs: Wave Plan, Handoffs, Messages", () => {
    renderOrchestrate();
    expect(screen.getByRole("tab", { name: /Wave Plan/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Handoffs/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Messages/ })).toBeInTheDocument();
  });

  it("shows Wave Plan tab as active by default", () => {
    renderOrchestrate();
    const wavePlanTab = screen.getByRole("tab", { name: /Wave Plan/ });
    expect(wavePlanTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText(/Create Wave Plan/i)).toBeInTheDocument();
  });

  it("switches to Handoffs panel on tab click", async () => {
    renderOrchestrate();
    await userEvent.click(screen.getByRole("tab", { name: /Handoffs/ }));
    expect(screen.getByText(/Build Handoff Envelope/i)).toBeInTheDocument();
  });

  it("switches to Messages panel on tab click", async () => {
    renderOrchestrate();
    await userEvent.click(screen.getByRole("tab", { name: /Messages/ }));
    expect(screen.getByText(/Parse ACB Signal/i)).toBeInTheDocument();
  });

  it("opens Handoffs tab when navigating to /orchestrate/handoffs", () => {
    renderOrchestrate(["/orchestrate/handoffs"]);
    expect(screen.getByRole("tab", { name: /Handoffs/ })).toHaveAttribute("aria-selected", "true");
  });

  it("shows active handoff count badge on Handoffs tab", () => {
    renderOrchestrate();
    const handoffsTab = screen.getByRole("tab", { name: /Handoffs/ });
    expect(handoffsTab).toBeInTheDocument();
  });

  it("shows open signal count badge on Messages tab", () => {
    renderOrchestrate();
    const messagesTab = screen.getByRole("tab", { name: /Messages/ });
    expect(messagesTab).toBeInTheDocument();
  });

  it("loads_plans_and_executes_wave_from_ui", async () => {
    const plan = {
      id: "plan-billing",
      project_id: "acc-main",
      slug: "add-billing",
      docs_path: null,
      status: "planning",
      created_at: "2026-11-20T00:00:00Z",
      completed_at: null,
    };
    const agent = {
      id: "pa-1",
      plan_id: "plan-billing",
      agent_ref: "1-1",
      task: "Create billing schema",
      wave: 1,
      depends_on: null,
      agent_id: null,
      status: "queued",
      guideline_path: null,
      handoff_path: null,
      started_at: null,
      completed_at: null,
      retry_count: 0,
    };
    const waveReport = {
      plan_id: "plan-billing",
      base_repo: ".",
      agents: [
        {
          agent_ref: "1-1",
          session_id: "sess-1",
          worktree_path: ".worktrees/plan-billing-1-1",
          branch: "agent/plan-billing-1-1",
          status: "running",
          guideline_path: ".acc/GUIDELINE.md",
          cost_usd: 0.05,
        },
      ],
      started_at: "2026-11-20T00:00:00Z",
      completed_at: null,
      total_cost_usd: 0.05,
    };
    const verifyResult = {
      wave: {
        ...waveReport,
        agents: [{ ...waveReport.agents[0], status: "done" }],
        completed_at: "2026-11-20T00:01:00Z",
      },
      verification: {
        passed: true,
        checks: [{ name: "Build Check", status: "Pass", detail: "vite build ok" }],
      },
    };

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wave_plans_cmd") return [plan];
      if (cmd === "get_plan_agents_cmd") return [agent];
      if (cmd === "get_corrections_cmd") return [];
      if (cmd === "execute_wave_cmd") return waveReport;
      if (cmd === "verify_and_finalize_wave_cmd") return verifyResult;
      return [];
    });

    render(
      <MemoryRouter initialEntries={["/orchestrate"]}>
        <ThemeProvider>
          <Orchestrate />
        </ThemeProvider>
      </MemoryRouter>
    );

    const planBadge = await screen.findByText("add-billing");
    expect(mockInvoke).toHaveBeenCalledWith("get_wave_plans_cmd", { projectId: null });

    await userEvent.click(planBadge);
    const executeBtn = await screen.findByRole("button", { name: /Execute Wave Plan/i });
    await userEvent.click(executeBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("execute_wave_cmd", {
        planId: "plan-billing",
        baseRepo: ".",
        baseBranch: "main",
        agentCommand: "opencode",
        agentBaseArgs: [],
        deadlineSecs: null,
        costCapUsd: null,
      });
    });

    expect(await screen.findByText(/agent\/plan-billing-1-1/i)).toBeInTheDocument();

    const verifyBtn = screen.getByRole("button", { name: /Verify & Finalize/i });
    await userEvent.click(verifyBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("verify_and_finalize_wave_cmd", {
        report: waveReport,
        projectPath: ".",
      });
    });

    expect(await screen.findByText(/Build Check/i)).toBeInTheDocument();
  });

  it("auto-defaults Create Wave Plan Project ID from currentProject or acc-main", () => {
    renderOrchestrate();
    const projectInput = screen.getByLabelText("Project ID") as HTMLInputElement;
    expect(projectInput.value).toBe("acc-main");

    useProjectStore.setState({
      currentProject: {
        id: "proj-custom",
        path: "/repo/custom",
        name: "custom",
        stack: [],
        active_agents: [],
        active_skills: [],
        active_mcps: [],
        preferred_models: [],
      },
    });
    renderOrchestrate();
    const inputs = screen.getAllByLabelText("Project ID") as HTMLInputElement[];
    expect(inputs[1].value).toBe("proj-custom");
  });

  it("renders Agent ID select, Depends On select, and numeric Wave # input in Add Plan Agent", async () => {
    const plan = {
      id: "plan-1",
      project_id: "acc-main",
      slug: "ux-plan",
      docs_path: null,
      status: "planning",
      created_at: "2026-11-20T00:00:00Z",
      completed_at: null,
    };
    const existingAgents = [
      {
        id: "pa-1",
        plan_id: "plan-1",
        agent_ref: "1-1",
        task: "First task",
        wave: 1,
        depends_on: null,
        agent_id: "opencode",
        status: "queued",
        guideline_path: null,
        handoff_path: null,
        started_at: null,
        completed_at: null,
        retry_count: 0,
      },
      {
        id: "pa-2",
        plan_id: "plan-1",
        agent_ref: "1-2",
        task: "Second task",
        wave: 2,
        depends_on: "1-1",
        agent_id: "claude",
        status: "queued",
        guideline_path: null,
        handoff_path: null,
        started_at: null,
        completed_at: null,
        retry_count: 0,
      },
    ];

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wave_plans_cmd") return [plan];
      if (cmd === "get_plan_agents_cmd") return existingAgents;
      if (cmd === "get_corrections_cmd") return [];
      return [];
    });

    render(
      <MemoryRouter initialEntries={["/orchestrate"]}>
        <ThemeProvider>
          <Orchestrate />
        </ThemeProvider>
      </MemoryRouter>
    );

    const planBadge = await screen.findByText("ux-plan");
    await userEvent.click(planBadge);

    const agentIdSelect = (await screen.findByLabelText("Agent ID")) as HTMLSelectElement;
    const agentIdValues = Array.from(agentIdSelect.options).map((o) => o.value);
    expect(agentIdValues).toEqual([
      "",
      "pi",
      "opencode",
      "opencode-acp",
      "claude",
      "codex",
      "gemini",
      "aider",
      "goose",
      "cline",
      "cursor",
      "qwen-code",
    ]);
    expect(agentIdSelect.options[0].textContent).toBe("Default (Wave Harness)");

    const dependsOnSelect = screen.getByLabelText("Depends On") as HTMLSelectElement;
    const dependsOnLabels = Array.from(dependsOnSelect.options).map((o) => o.textContent);
    expect(dependsOnLabels).toEqual([
      "None (no dependency)",
      "1-1 (Wave 1)",
      "1-2 (Wave 2)",
    ]);

    const waveInput = screen.getByLabelText("Wave #") as HTMLInputElement;
    expect(waveInput).toHaveAttribute("type", "number");
    expect(waveInput).toHaveAttribute("min", "1");
    expect(waveInput).toHaveAttribute("max", "20");

    await userEvent.type(screen.getByPlaceholderText("e.g., claude"), "2-1");
    await userEvent.selectOptions(agentIdSelect, "claude");
    await userEvent.type(screen.getByPlaceholderText("What should this agent do?"), "Implement UI");
    await userEvent.clear(waveInput);
    await userEvent.type(waveInput, "2");
    await userEvent.selectOptions(dependsOnSelect, "1-1");
    await userEvent.click(screen.getByRole("button", { name: /Add Agent/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("add_plan_agent_cmd", {
        planId: "plan-1",
        agentRef: "2-1",
        task: "Implement UI",
        wave: 2,
        dependsOn: "1-1",
        agentId: "claude",
      });
    });
  });
});
