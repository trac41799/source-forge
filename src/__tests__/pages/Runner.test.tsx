import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import Runner from "@/pages/Runner";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

function renderRunner() {
  mockInvoke.mockResolvedValue([]);
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <Runner />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Runner Page", () => {
  it("renders AGENTS section", () => {
    renderRunner();
    expect(screen.getByText("AGENTS")).toBeInTheDocument();
  });

  it("renders Add Agent button", () => {
    renderRunner();
    expect(screen.getByText("+ Add Agent")).toBeInTheDocument();
  });

  it("renders PRESETS section", () => {
    renderRunner();
    expect(screen.getByText("PRESETS")).toBeInTheDocument();
  });

  it("renders + New preset button", () => {
    renderRunner();
    expect(screen.getByText("+ New")).toBeInTheDocument();
  });

  it("renders project selector", () => {
    renderRunner();
    expect(screen.getByText("Project:")).toBeInTheDocument();
  });

  it("renders Control Mode button", () => {
    renderRunner();
    expect(screen.getByText("Control Mode")).toBeInTheDocument();
  });

  it("renders Load Profile button", () => {
    renderRunner();
    expect(screen.getByText("Load Profile")).toBeInTheDocument();
  });

  it("renders session footer with agent count", () => {
    renderRunner();
    expect(screen.getByText("0 agents")).toBeInTheDocument();
  });

  it("renders session footer with files changed", () => {
    renderRunner();
    expect(screen.getByText("0 files changed")).toBeInTheDocument();
  });

  it("renders Analyze button in footer", () => {
    renderRunner();
    expect(screen.getByText("Analyze")).toBeInTheDocument();
  });

  it("renders Docs button in footer", () => {
    renderRunner();
    expect(screen.getByText("Docs")).toBeInTheDocument();
  });

  it("lists_all_configured_agents_and_wires_actions", async () => {
    const user = userEvent.setup({ delay: null });
    renderRunner();

    await user.click(screen.getByText("+ Add Agent"));
    for (const label of [
      "Claude Code",
      "OpenCode",
      "Aider",
      "Goose",
      "Cline CLI",
      "Cursor",
      "Gemini CLI",
      "Qwen Code",
      "Codex CLI",
    ]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }

    // Load Profile invokes generate_profile
    await user.click(screen.getByText("Load Profile"));
    expect(mockInvoke).toHaveBeenCalledWith("generate_profile", {
      projectPath: ".",
    });

    // + New opens inline preset creator and adds preset
    await user.click(screen.getByText("+ New"));
    const labelInput = screen.getByPlaceholderText("Preset label");
    const cmdInput = screen.getByPlaceholderText("Command");
    await user.type(labelInput, "Build Check");
    await user.type(cmdInput, "npm run build");
    await user.click(screen.getByText("Save Preset"));
    expect(screen.getByText("Build Check")).toBeInTheDocument();

    // Analyze triggers repo map build
    await user.click(screen.getByText("Analyze"));
    expect(mockInvoke).toHaveBeenCalledWith(
      "build_repo_map_cmd",
      expect.objectContaining({
        projectPath: ".",
      })
    );

    // Docs button triggers navigation or docs view
    await user.click(screen.getByText("Docs"));
  }, 15000);

  it("shows control sessions section when control mode is active", async () => {
    const user = userEvent.setup();
    renderRunner();
    await user.click(screen.getByText("Control Mode"));
    expect(screen.getByText("CONTROL SESSIONS")).toBeInTheDocument();
  });

  it("shows project select dropdown", () => {
    renderRunner();
    const select = screen.getByRole("combobox");
    expect(select).toBeInTheDocument();
  });
});
