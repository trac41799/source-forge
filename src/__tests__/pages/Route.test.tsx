import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import RoutePage from "@/pages/Route";
import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { useProjectStore } from "@/stores/projectStore";
import { useAgentStore } from "@/stores/agentStore";

const mockNavigate = vi.fn();

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

beforeEach(() => {
  mockNavigate.mockReset();
  useProjectStore.setState({
    currentProject: null,
    recentProjects: [],
    recentPaths: [],
  });
  useAgentStore.setState({
    agents: new Map(),
  });
  useOrchestrationStore.setState({
    suggestions: [],
  });
});

function renderRoute() {
  mockInvoke.mockResolvedValue([]);
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <RoutePage />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Route Page", () => {
  it("renders Project Scope select with All Projects, active project, and recent projects", async () => {
    useProjectStore.setState({
      currentProject: {
        id: "proj-1",
        path: "/code/myapp",
        name: "myapp",
        stack: [],
        active_agents: [],
        active_skills: [],
        active_mcps: [],
        preferred_models: [],
      },
      recentProjects: ["/code/myapp", "/code/other"],
      recentPaths: ["/code/myapp", "/code/other"],
    });

    renderRoute();

    const scopeSelect = screen.getByLabelText("Project Scope") as HTMLSelectElement;
    const optionValues = Array.from(scopeSelect.options).map((o) => o.value);
    expect(optionValues).toEqual(["", "proj-1", "/code/other"]);
    expect(scopeSelect.options[0].textContent).toBe("All Projects (Cross-project routing)");

    await userEvent.type(screen.getByPlaceholderText(/Describe the task/i), "Add OAuth");
    await userEvent.selectOptions(scopeSelect, "proj-1");
    await userEvent.click(screen.getByRole("button", { name: /Route Task/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("route_task_cmd", {
        taskDesc: "Add OAuth",
        taskType: "implement",
        projectId: "proj-1",
      });
    });
  });

  it("spawns suggested agent via useAgentStore and navigates to /runner on Send to Agent", async () => {
    useProjectStore.setState({
      currentProject: {
        id: "proj-1",
        path: "/code/myapp",
        name: "myapp",
        stack: [],
        active_agents: [],
        active_skills: [],
        active_mcps: [],
        preferred_models: [],
      },
    });
    useOrchestrationStore.setState({
      suggestions: [
        {
          agent_id: "claude",
          model_id: "sonnet-4",
          confidence: 0.92,
          reasoning: "Best historical pass rate for implement tasks",
          success_rate: 0.95,
        },
      ],
    });

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "check_agent_installed") return { installed: true, install_hint: null };
      if (cmd === "spawn_agent") return undefined;
      return [];
    });

    render(
      <MemoryRouter>
        <ThemeProvider>
          <RoutePage />
        </ThemeProvider>
      </MemoryRouter>
    );

    await userEvent.click(screen.getByRole("button", { name: /Send to Agent/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "spawn_agent",
        expect.objectContaining({
          agentId: "claude",
          projectPath: "/code/myapp",
          command: "claude",
        })
      );
    });

    expect(useAgentStore.getState().agents.has("claude")).toBe(true);
    expect(mockNavigate).toHaveBeenCalledWith("/runner");
  });
});
