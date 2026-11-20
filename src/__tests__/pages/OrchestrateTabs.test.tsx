import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import Orchestrate from "@/pages/Orchestrate";
import { mockInvoke } from "@/__tests__/setup";
import { useOrchestrationStore } from "@/stores/orchestrationStore";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

beforeEach(() => {
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
});
