import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { Sidebar } from "@/components/layout/Sidebar";
import { useSettingsStore } from "@/stores/settingsStore";

function renderSidebar(initialEntries = ["/"]) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <ThemeProvider>
        <Sidebar />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Sidebar keyboard shortcuts", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      sidebarCollapsed: { workspace: false, knowledge: true, review: true, settings: true },
    });
    delete (window as any).location;
    (window as any).location = { href: "" };
  });

  it("Ctrl+1 navigates to /runner", async () => {
    renderSidebar();
    await userEvent.keyboard("{Control>}1{/Control}");
    expect(window.location.href).toBe("/runner");
  });

  it("Ctrl+2 navigates to /orchestrate", async () => {
    renderSidebar();
    await userEvent.keyboard("{Control>}2{/Control}");
    expect(window.location.href).toBe("/orchestrate");
  });

  it("Ctrl+, navigates to /settings", async () => {
    renderSidebar();
    await userEvent.keyboard("{Control>},{/Control}");
    expect(window.location.href).toBe("/settings");
  });

  it("Enter key toggles a collapsed group", async () => {
    renderSidebar();
    const reviewHeader = screen.getByRole("button", { name: /Review & Outcomes/ });
    reviewHeader.focus();
    expect(reviewHeader).toHaveAttribute("aria-expanded", "false");
    await userEvent.keyboard("{Enter}");
    expect(reviewHeader).toHaveAttribute("aria-expanded", "true");
  });

  it("supports Tab key navigation: Runner first, then Workspace group", async () => {
    renderSidebar();
    await userEvent.tab();
    expect(screen.getByRole("link", { name: /Runner/ })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: /Workspace/ })).toHaveFocus();
  });
});
