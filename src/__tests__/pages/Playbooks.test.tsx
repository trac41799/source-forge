import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import Playbooks from "@/pages/Playbooks";
import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(open).mockReset();
  useOrchestrationStore.setState({
    memoryCandidates: [],
    playbookManifest: null,
  });
});

function renderPlaybooks() {
  mockInvoke.mockResolvedValue([]);
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <Playbooks />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Playbooks Page", () => {
  it("renders page title", () => {
    renderPlaybooks();
    expect(screen.getByText("Playbooks")).toBeInTheDocument();
  });

  it("renders description", () => {
    renderPlaybooks();
    expect(screen.getByText(/Export and import .acc bundles/)).toBeInTheDocument();
  });

  it("renders Export Playbook section", () => {
    renderPlaybooks();
    expect(screen.getByText("Export Playbook")).toBeInTheDocument();
  });

  it("renders Import Playbook section", () => {
    renderPlaybooks();
    expect(screen.getByText("Import Playbook")).toBeInTheDocument();
  });

  it("renders Playbook Name input", () => {
    renderPlaybooks();
    expect(screen.getByPlaceholderText("my-client-playbook")).toBeInTheDocument();
  });

  it("renders include checkboxes", () => {
    renderPlaybooks();
    expect(screen.getByText("Skills Library")).toBeInTheDocument();
    expect(screen.getByText("Memory Files")).toBeInTheDocument();
    expect(screen.getByText("Preset Commands")).toBeInTheDocument();
  });

  it("renders Generate Manifest button", () => {
    renderPlaybooks();
    expect(screen.getByText("Generate Manifest")).toBeInTheDocument();
  });

  it("renders drop zone for .acc files", () => {
    renderPlaybooks();
    expect(screen.getByText(/Drop a .acc playbook file here/)).toBeInTheDocument();
  });

  it("renders Select .acc File button", () => {
    renderPlaybooks();
    expect(screen.getByText("Select .acc File")).toBeInTheDocument();
  });

  it("renders Reactive Memory section", () => {
    renderPlaybooks();
    expect(screen.getByText("Reactive Memory")).toBeInTheDocument();
  });

  it("renders Feature Docs section", () => {
    renderPlaybooks();
    expect(screen.getByText("Feature Docs")).toBeInTheDocument();
  });

  it("renders doc type buttons", () => {
    renderPlaybooks();
    expect(screen.getByText("EXECUTIVE PLAN")).toBeInTheDocument();
    expect(screen.getByText("CHANGELOG")).toBeInTheDocument();
    expect(screen.getByText("QA REPORT")).toBeInTheDocument();
    expect(screen.getByText("TECHNICAL PLAN")).toBeInTheDocument();
  });

  it("renders empty memory candidates message", () => {
    renderPlaybooks();
    expect(screen.getByText(/No memory candidates yet/)).toBeInTheDocument();
  });

  it("opens .acc file picker on Select .acc File click and displays imported path", async () => {
    vi.mocked(open).mockResolvedValueOnce("C:/bundles/team.acc");
    renderPlaybooks();

    await userEvent.click(screen.getByRole("button", { name: /Select \.acc File/i }));

    await waitFor(() => {
      expect(open).toHaveBeenCalledWith({
        directory: false,
        filters: [{ name: "ACC Playbook", extensions: ["acc", "json", "zip"] }],
      });
    });

    expect(await screen.findByText(/C:\/bundles\/team\.acc/)).toBeInTheDocument();
  });

  it("updates candidate status and feedback when Add to Memory or Skip is clicked", async () => {
    const candidates = [
      {
        id: "cand-1",
        session_id: "sess-1",
        project_id: "proj-1",
        content: "Always run tsc --noEmit before committing",
        source_pattern: "tsc-check",
        status: "pending",
        created_at: "2026-11-20T00:00:00Z",
      },
      {
        id: "cand-2",
        session_id: "sess-1",
        project_id: "proj-1",
        content: "Temporary debug note",
        source_pattern: null,
        status: "pending",
        created_at: "2026-11-20T00:00:00Z",
      },
    ];

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_memory_candidates_cmd") return candidates;
      if (cmd === "create_memory_fact_cmd") {
        return {
          id: "fact-1",
          agent_id: "playbook",
          session_id: "sess-1",
          user_id: "local",
          org_id: "proj-1",
          fact_type: "learning",
          content: "Always run tsc --noEmit before committing",
          confidence: 0.9,
          created_at: "2026-11-20T00:00:00Z",
        };
      }
      return [];
    });

    render(
      <MemoryRouter>
        <ThemeProvider>
          <Playbooks />
        </ThemeProvider>
      </MemoryRouter>
    );

    expect(await screen.findByText("Always run tsc --noEmit before committing")).toBeInTheDocument();

    const addButtons = screen.getAllByRole("button", { name: /Add to Memory/i });
    const skipButtons = screen.getAllByRole("button", { name: /Skip/i });

    await userEvent.click(addButtons[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("create_memory_fact_cmd", {
        input: expect.objectContaining({
          content: "Always run tsc --noEmit before committing",
          session_id: "sess-1",
          org_id: "proj-1",
        }),
      });
    });

    expect(await screen.findByText("approved")).toBeInTheDocument();
    expect(screen.getByText(/Added to memory/i)).toBeInTheDocument();

    await userEvent.click(skipButtons[1]);
    expect(await screen.findByText("skipped")).toBeInTheDocument();
    expect(screen.getByText("Skipped")).toBeInTheDocument();
  });
});
