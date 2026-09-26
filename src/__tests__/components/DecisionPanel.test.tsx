import { render, screen, waitFor } from "@testing-library/react";
import { DecisionPanel } from "@/components/DecisionPanel";
import { mockInvoke } from "../setup";

describe("DecisionPanel", () => {
  it("renders backend health and the review queue", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_decision_config_cmd") {
        return Promise.resolve({
          backend: "hosted",
          base_url: "https://openrouter.ai/api",
          model: "typesafe/jev-1.13",
          accept_threshold: 0.75,
          review_threshold: 0.4,
          context_limit: 32000,
          timeout_ms: 5000,
        });
      }
      if (cmd === "decision_health_cmd") return Promise.resolve("healthy");
      if (cmd === "list_decision_reviews_cmd") {
        return Promise.resolve([
          { id: "r1", consumer: "router", decided_value: "none", confidence: 0.5, resolved: false },
        ]);
      }
      return Promise.resolve(null);
    });

    render(<DecisionPanel />);

    await waitFor(() =>
      expect(screen.getByTestId("decision-health")).toHaveTextContent("healthy")
    );
    expect(screen.getByTestId("decision-reviews")).toHaveTextContent("router");
    expect(screen.getByText(/Review queue \(1\)/)).toBeInTheDocument();
  });
});
