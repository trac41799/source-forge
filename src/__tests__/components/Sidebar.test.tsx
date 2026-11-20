import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { Sidebar } from "@/components/layout/Sidebar";
import { useSettingsStore } from "@/stores/settingsStore";

vi.mock("@/stores/knowledgeStore", () => ({
  useKnowledgeStore: vi.fn((selector) => {
    const state = { newItemsSinceLastVisit: 3 };
    return selector(state);
  }),
}));

function renderSidebar(initialEntries = ["/"]) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <ThemeProvider>
        <Sidebar />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Sidebar", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      experimentalFeatures: false,
      sidebarCollapsed: { workspace: false, knowledge: true, review: true, settings: true },
    });
  });

  it("renders_four_core_groups_and_gates_experimental", () => {
    renderSidebar();
    const groups = screen.getAllByRole("button", {
      name: /^(Workspace|Knowledge & Assets|Review & Outcomes|Settings)/,
    });
    expect(groups).toHaveLength(4);
    expect(groups[0]).toHaveTextContent("Workspace");
    expect(groups[1]).toHaveTextContent("Knowledge & Assets");
    expect(groups[2]).toHaveTextContent("Review & Outcomes");
    expect(groups[3]).toHaveTextContent("Settings");
    expect(useSettingsStore.getState().experimentalFeatures).toBe(false);
  });

  it("renders Runner as a persistent top item outside any group", () => {
    renderSidebar();
    const runner = screen.getByRole("link", { name: /Runner/ });
    expect(runner).toBeInTheDocument();
    expect(runner.closest("[data-collapsible]")).toBeNull();
  });

  it("expands Workspace group by default", () => {
    renderSidebar();
    expect(screen.getByRole("link", { name: /Orchestrate/ })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Build App/ })).toBeInTheDocument();
  });

  it("collapses Knowledge & Assets, Review & Outcomes, Settings groups by default", () => {
    renderSidebar();
    const knowledgeBtn = screen.getByRole("button", { name: /Knowledge & Assets/ });
    expect(knowledgeBtn).toHaveAttribute("aria-expanded", "false");
    const reviewBtn = screen.getByRole("button", { name: /Review & Outcomes/ });
    expect(reviewBtn).toHaveAttribute("aria-expanded", "false");
    const settingsBtn = screen.getByRole("button", { name: /^Settings$/ });
    expect(settingsBtn).toHaveAttribute("aria-expanded", "false");
  });

  it("expands a collapsed group on header click", async () => {
    renderSidebar();
    const reviewHeader = screen.getByRole("button", { name: /Review & Outcomes/ });
    expect(reviewHeader).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(reviewHeader);
    expect(reviewHeader).toHaveAttribute("aria-expanded", "true");
  });

  it("collapses an expanded group on header click", async () => {
    renderSidebar();
    const workspaceHeader = screen.getByRole("button", { name: /Workspace/ });
    expect(workspaceHeader).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(workspaceHeader);
    expect(workspaceHeader).toHaveAttribute("aria-expanded", "false");
  });

  it("behaves as accordion — expanding one group collapses others", async () => {
    useSettingsStore.setState({
      sidebarCollapsed: { workspace: false, knowledge: true, review: true, settings: true },
    });
    renderSidebar();
    await userEvent.click(screen.getByRole("button", { name: /Review & Outcomes/ }));
    await userEvent.click(screen.getByRole("button", { name: /Knowledge & Assets/ }));
    const knowledgeBtn = screen.getByRole("button", { name: /Knowledge & Assets/ });
    expect(knowledgeBtn).toHaveAttribute("aria-expanded", "true");
    const reviewBtn = screen.getByRole("button", { name: /Review & Outcomes/ });
    expect(reviewBtn).toHaveAttribute("aria-expanded", "false");
  });

  it("auto-expands the group containing the active route", () => {
    renderSidebar(["/outcomes"]);
    const reviewBtn = screen.getByRole("button", { name: /Review & Outcomes/ });
    expect(reviewBtn).toHaveAttribute("aria-expanded", "true");
  });

  it("shows new-items badge on collapsed Knowledge & Assets group header", () => {
    useSettingsStore.setState({
      sidebarCollapsed: { workspace: true, knowledge: true, review: true, settings: true },
    });
    renderSidebar();
    const knowledgeHeader = screen.getByRole("button", { name: /Knowledge & Assets/ });
    expect(knowledgeHeader).toHaveTextContent("3");
  });

  it("handles Ctrl+1 keyboard shortcut", async () => {
    delete (window as any).location;
    (window as any).location = { href: "" };
    renderSidebar();
    await userEvent.keyboard("{Control>}1{/Control}");
    expect(window.location.href).toBe("/runner");
  });
});
