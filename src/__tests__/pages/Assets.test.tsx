import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import { useProjectStore } from "@/stores/projectStore";
import { open } from "@tauri-apps/plugin-dialog";
import Assets from "@/pages/Assets";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

function renderAssets() {
  mockInvoke.mockResolvedValue([]);
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <Assets />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Assets Page", () => {
  it("renders page title", () => {
    renderAssets();
    expect(screen.getByText("Asset Manager")).toBeInTheDocument();
  });

  it("renders all tab buttons", () => {
    renderAssets();
    expect(screen.getByText("Skills Library")).toBeInTheDocument();
    expect(screen.getByText("Memory Browser")).toBeInTheDocument();
    expect(screen.getByText("MCP Registry")).toBeInTheDocument();
    expect(screen.getByText("Connector Vault")).toBeInTheDocument();
    expect(screen.getByText("Plugin Manager")).toBeInTheDocument();
  });

  it("renders search input", () => {
    renderAssets();
    expect(screen.getByPlaceholderText("Search...")).toBeInTheDocument();
  });

  it("renders Scan button for skills tab", () => {
    renderAssets();
    const scanButtons = screen.getAllByText("Scan");
    expect(scanButtons.length).toBeGreaterThan(0);
  });

  it("renders footer with count", () => {
    renderAssets();
    expect(screen.getByText("0 skills")).toBeInTheDocument();
  });

  it("renders empty state for skills", () => {
    renderAssets();
    expect(screen.getByText(/No skills found/)).toBeInTheDocument();
  });

  it("auto-defaults scanPath and profilePath from active project", async () => {
    useProjectStore.setState({
      currentProject: {
        id: "p1",
        path: "/workspace/my-app",
        name: "my-app",
        stack: [],
        active_agents: [],
        active_skills: [],
        active_mcps: [],
        preferred_models: [],
      },
    });
    renderAssets();
    expect(screen.getByPlaceholderText("Scan path...")).toHaveValue("/workspace/my-app");

    await userEvent.click(screen.getByText("Memory Browser"));
    expect(screen.getByPlaceholderText("Project path...")).toHaveValue("/workspace/my-app");

    await userEvent.click(screen.getByText("Plugin Manager"));
    expect(screen.getByPlaceholderText("Project path...")).toHaveValue("/workspace/my-app");
    useProjectStore.setState({ currentProject: null });
  });

  it("picks folder via Browse Folder on skills, memory, and plugins tabs", async () => {
    useProjectStore.setState({ currentProject: null });
    const mockOpen = vi.mocked(open);
    mockOpen.mockReset();
    renderAssets();

    mockOpen.mockResolvedValueOnce("/picked/skills");
    await userEvent.click(screen.getByRole("button", { name: /Browse Folder/i }));
    expect(mockOpen).toHaveBeenCalledWith(expect.objectContaining({ directory: true }));
    expect(screen.getByPlaceholderText("Scan path...")).toHaveValue("/picked/skills");

    await userEvent.click(screen.getByText("Memory Browser"));
    mockOpen.mockResolvedValueOnce("/picked/memory");
    await userEvent.click(screen.getByRole("button", { name: /Browse Folder/i }));
    expect(screen.getByPlaceholderText("Project path...")).toHaveValue("/picked/memory");

    await userEvent.click(screen.getByText("Plugin Manager"));
    mockOpen.mockResolvedValueOnce("/picked/plugins");
    await userEvent.click(screen.getByRole("button", { name: /Browse Folder/i }));
    expect(screen.getByPlaceholderText("Project path...")).toHaveValue("/picked/plugins");
  });

  it("provides Browse File and preset selector on MCP Registry tab", async () => {
    const mockOpen = vi.mocked(open);
    mockOpen.mockReset();
    renderAssets();

    await userEvent.click(screen.getByText("MCP Registry"));
    const presetSelect = screen.getByLabelText(/MCP config preset/i);
    fireEvent.change(presetSelect, { target: { value: "~/.claude.json" } });
    expect(screen.getByPlaceholderText(/MCP config path/i)).toHaveValue("~/.claude.json");

    expect(screen.getByRole("option", { name: /\.opencode\/config\.json/ })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /~\/\.gemini\/settings\.json/ })).toBeInTheDocument();

    mockOpen.mockResolvedValueOnce("/custom/mcp.json");
    await userEvent.click(screen.getByRole("button", { name: /Browse File/i }));
    expect(mockOpen).toHaveBeenCalledWith(
      expect.objectContaining({
        directory: false,
        filters: [{ name: "Config", extensions: ["json", "yaml", "yml", "toml"] }],
      })
    );
    expect(screen.getByPlaceholderText(/MCP config path/i)).toHaveValue("/custom/mcp.json");
  });

  it("provides quick-pick key chips and Agent Scope dropdown in Connector Vault", async () => {
    renderAssets();
    await userEvent.click(screen.getByText("Connector Vault"));
    await userEvent.click(screen.getByRole("button", { name: /Add Secret/i }));

    for (const keyName of [
      "OPENROUTER_API_KEY",
      "ANTHROPIC_API_KEY",
      "OPENAI_API_KEY",
      "GEMINI_API_KEY",
    ]) {
      expect(screen.getByRole("button", { name: keyName })).toBeInTheDocument();
    }

    await userEvent.click(screen.getByRole("button", { name: "ANTHROPIC_API_KEY" }));
    expect(screen.getByPlaceholderText("e.g. OPENAI_API_KEY")).toHaveValue("ANTHROPIC_API_KEY");

    const agentSelect = screen.getByLabelText(/Agent Scope/i);
    expect(agentSelect.tagName).toBe("SELECT");
    expect(screen.getByRole("option", { name: "All Agents (Global)" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Pi/i })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Claude Code/i })).toBeInTheDocument();
  });
});
