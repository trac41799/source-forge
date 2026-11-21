import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import KnowledgePage from "@/pages/Knowledge";
import { useKnowledgeStore } from "@/stores/knowledgeStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("cytoscape", () => ({
  default: vi.fn(() => ({
    on: vi.fn(),
    destroy: vi.fn(),
    edges: vi.fn(() => ({ style: vi.fn() })),
  })),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

function renderKnowledge() {
  mockInvoke.mockResolvedValue([]);
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <KnowledgePage />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("KnowledgePage", () => {
  it("renders Knowledge Compounder page title", () => {
    renderKnowledge();
    expect(screen.getByText("Knowledge Compounder")).toBeInTheDocument();
  });

  it("renders Run Compounder button", () => {
    renderKnowledge();
    expect(screen.getByText("Run Compounder")).toBeInTheDocument();
  });

  it("renders Browse tab", () => {
    renderKnowledge();
    expect(screen.getByText("Browse")).toBeInTheDocument();
  });

  it("renders Refresh button", () => {
    renderKnowledge();
    expect(screen.getByText("Refresh")).toBeInTheDocument();
  });

  it("renders Add Item button", () => {
    renderKnowledge();
    expect(screen.getByText("Add Item")).toBeInTheDocument();
  });

  it("renders search input", () => {
    renderKnowledge();
    expect(screen.getByPlaceholderText(/search/i)).toBeInTheDocument();
  });

  it("Run Compounder button click opens dialog", async () => {
    renderKnowledge();
    const btn = screen.getByText("Run Compounder");
    await userEvent.click(btn);
    expect(screen.getByText(/Session ID/i)).toBeInTheDocument();
  });

  it("renders Relations tab", () => {
    renderKnowledge();
    expect(screen.getByText("Relations")).toBeInTheDocument();
  });

  it("gates Bagua hexagram/trigram controls behind experimentalFeatures", async () => {
    useKnowledgeStore.setState({
      kgSubgraph: {
        nodes: [{ id: "n1", title: "Pattern A", type: "pattern", confidence: 0.9, depth: 0 }],
        edges: [],
      },
    });

    useSettingsStore.setState({ experimentalFeatures: false });
    const { rerender } = renderKnowledge();
    await userEvent.click(screen.getByText("KG Explorer"));
    expect(screen.queryByText("Bagua Legend")).not.toBeInTheDocument();
    expect(screen.queryByText(/Trigram filter/i)).not.toBeInTheDocument();

    useSettingsStore.setState({ experimentalFeatures: true });
    rerender(
      <MemoryRouter>
        <ThemeProvider>
          <KnowledgePage />
        </ThemeProvider>
      </MemoryRouter>
    );
    expect(screen.getByText("Bagua Legend")).toBeInTheDocument();
    expect(screen.getByText(/Trigram filter/i)).toBeInTheDocument();
  });

  it("renders Agent filter as a select dropdown with All Agents, pi, and AGENT_CONFIGS", async () => {
    renderKnowledge();
    const agentSelect = screen.getByLabelText(/^Agent$/i);
    expect(agentSelect.tagName).toBe("SELECT");
    expect(screen.getByRole("option", { name: "All Agents" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /^Pi$/i })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /OpenCode/i })).toBeInTheDocument();

    await userEvent.selectOptions(agentSelect, "opencode");
    expect(agentSelect).toHaveValue("opencode");
  });

  it("provides Browse File picker in Code Bridge and Co-change tabs", async () => {
    const mockOpen = vi.mocked(open);
    mockOpen.mockReset();
    renderKnowledge();

    await userEvent.click(screen.getByText("KG Explorer"));
    await userEvent.click(screen.getByRole("button", { name: /Code Bridge/i }));
    mockOpen.mockResolvedValueOnce("src/auth.ts");
    await userEvent.click(screen.getByRole("button", { name: /Browse File/i }));
    expect(mockOpen).toHaveBeenCalledWith(expect.objectContaining({ directory: false }));
    expect(screen.getByPlaceholderText(/Source file path/i)).toHaveValue("src/auth.ts");

    await userEvent.click(screen.getByRole("button", { name: /Co-change/i }));
    mockOpen.mockResolvedValueOnce("src/router.ts");
    await userEvent.click(screen.getByRole("button", { name: /Browse File/i }));
    expect(screen.getByPlaceholderText(/File path for co-change warnings/i)).toHaveValue(
      "src/router.ts"
    );
  });

  it("provides quick-select session pills in Run Compounder dialog", async () => {
    renderKnowledge();
    await userEvent.click(screen.getByText("Run Compounder"));

    for (const pill of ["sess-001", "sess-002", "latest"]) {
      expect(screen.getByRole("button", { name: pill })).toBeInTheDocument();
    }

    await userEvent.click(screen.getByRole("button", { name: "sess-002" }));
    expect(screen.getByPlaceholderText(/5a4b/i)).toHaveValue("sess-002");
  });
});
