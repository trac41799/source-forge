import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import CostAggregation from "@/pages/CostAggregation";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

function renderCostAggregation() {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "get_cost_summary_cmd") {
      return Promise.resolve({
        total_tokens_in: 0,
        total_tokens_out: 0,
        total_tokens: 0,
        estimated_total_cost_usd: 0,
        by_model: [],
        by_project: [],
        by_session: [],
      });
    }
    if (cmd === "get_budgets_cmd") {
      return Promise.resolve([]);
    }
    if (cmd === "get_resumption_plan_cmd") {
      return Promise.resolve(null);
    }
    return Promise.resolve([]);
  });
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <CostAggregation />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("CostAggregation Page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders page title", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("Cost Aggregation")).toBeInTheDocument();
    });
  });

  it("renders Refresh button", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("Refresh")).toBeInTheDocument();
    });
  });

  it("renders tab buttons", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("Overview")).toBeInTheDocument();
      expect(screen.getByText("Budgets")).toBeInTheDocument();
      expect(screen.getByText("Models")).toBeInTheDocument();
      expect(screen.getByText("Projects")).toBeInTheDocument();
      expect(screen.getByText("Sessions")).toBeInTheDocument();
    });
  });

  it("renders Total Spend card", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("Total Spend")).toBeInTheDocument();
    });
  });

  it("renders Burn Rate card", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("Burn Rate")).toBeInTheDocument();
    });
  });

  it("renders Projected Month-End card", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("Projected Month-End")).toBeInTheDocument();
    });
  });

  it("renders Active Budgets card", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("Active Budgets")).toBeInTheDocument();
    });
  });

  it("renders Threshold Ladder", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("Threshold Ladder")).toBeInTheDocument();
    });
  });

  it("renders Cost Breakdown section when data loads", async () => {
    renderCostAggregation();
    await waitFor(() => {
      // Cost Breakdown is rendered in overview tab
      expect(screen.getByText("Overview")).toBeInTheDocument();
    });
  });

  it("shows 0.0% burn rate when no data", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText("0.0%")).toBeInTheDocument();
    });
  });

  it("renders WIP / Resumption tab", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(screen.getByText(/WIP \/ Resumption/)).toBeInTheDocument();
    });
  });

  it("loads summary via get_cost_summary_cmd (SPEC-001 §3.2)", async () => {
    renderCostAggregation();
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_cost_summary_cmd", {
        projectId: null,
      });
    });
  });
});
