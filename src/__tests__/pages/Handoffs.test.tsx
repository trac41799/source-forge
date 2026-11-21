import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import Handoffs from "@/pages/Handoffs";
import { AGENT_CONFIGS } from "@/lib/agents/configs";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(open).mockReset();
});

function renderHandoffs() {
  mockInvoke.mockResolvedValue([]);
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <Handoffs />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Handoffs Page", () => {
  it("renders Completed By and Next Agent selects populated with AGENT_CONFIGS and Pi Agent (ACP)", () => {
    renderHandoffs();

    const completedBy = screen.getByLabelText("Completed By") as HTMLSelectElement;
    const nextAgent = screen.getByLabelText("Next Agent") as HTMLSelectElement;

    const expectedIds = ["", "pi", ...AGENT_CONFIGS.map((a) => a.id)];
    expect(Array.from(completedBy.options).map((o) => o.value)).toEqual(expectedIds);
    expect(Array.from(nextAgent.options).map((o) => o.value)).toEqual(expectedIds);

    expect(Array.from(completedBy.options).map((o) => o.textContent)).toContain("Pi Agent (ACP)");
    expect(Array.from(nextAgent.options).map((o) => o.textContent)).toContain("Pi Agent (ACP)");
  });

  it("browses handoff markdown file and populates validator and envelope", async () => {
    vi.mocked(open).mockResolvedValueOnce("/tmp/HANDOFF_1-1.md");
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "parse_handoff_file_cmd") {
        return {
          original_task: "Build auth",
          completed_by: "claude",
          model_used: "sonnet-4",
          output_summary: "Implemented auth service",
          changed_files: ["src/auth.ts"],
          handoff_instruction: "Verify edge cases",
          raw_path: "/tmp/HANDOFF_1-1.md",
        };
      }
      return [];
    });

    render(
      <MemoryRouter>
        <ThemeProvider>
          <Handoffs />
        </ThemeProvider>
      </MemoryRouter>
    );

    await userEvent.click(screen.getByRole("button", { name: /Browse Handoff File/i }));

    await waitFor(() => {
      expect(open).toHaveBeenCalledWith({
        directory: false,
        filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
      });
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("parse_handoff_file_cmd", {
        path: "/tmp/HANDOFF_1-1.md",
      });
    });

    expect(await screen.findByText(/\/tmp\/HANDOFF_1-1\.md/)).toBeInTheDocument();
    expect((screen.getByLabelText("Completed By") as HTMLSelectElement).value).toBe("claude");
    const validatorInput = screen.getByPlaceholderText(
      /Paste handoff content here to validate schema/i
    ) as HTMLTextAreaElement;
    expect(validatorInput.value).toContain("Implemented auth service");
    expect(validatorInput.value).toContain("## Completed Work");
    expect(validatorInput.value).toContain("## Handoff Instructions");
  });
});
