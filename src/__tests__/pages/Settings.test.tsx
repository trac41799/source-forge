import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import SettingsPage from "@/pages/Settings";
import { mockInvoke } from "../setup";

function renderSettings() {
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <SettingsPage />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("SettingsPage", () => {
  it("renders all sections", () => {
    renderSettings();
    expect(screen.getByText("Settings")).toBeInTheDocument();
    expect(screen.getByText("Appearance")).toBeInTheDocument();
    expect(screen.getByText("Defaults")).toBeInTheDocument();
    expect(screen.getByText("Integrations")).toBeInTheDocument();
    expect(screen.getByText("About")).toBeInTheDocument();
  });

  it("renders theme radio buttons", () => {
    renderSettings();
    expect(screen.getByLabelText("Dark")).toBeInTheDocument();
    expect(screen.getByLabelText("Light")).toBeInTheDocument();
    expect(screen.getByLabelText("System")).toBeInTheDocument();
  });

  it("renders default agent/model selects", () => {
    renderSettings();
    expect(screen.getByDisplayValue("OpenCode")).toBeInTheDocument();
  });

  it("renders integration status cards", () => {
    renderSettings();
    expect(screen.getByText("SkillBridge")).toBeInTheDocument();
    expect(screen.getByText("Supabase")).toBeInTheDocument();
    expect(screen.getByText("GitHub")).toBeInTheDocument();
  });

  it("renders about section with version", () => {
    renderSettings();
    expect(screen.getByText("SourceForge")).toBeInTheDocument();
    expect(screen.getByText("0.9.0")).toBeInTheDocument();
  });

  it("Save Defaults button is present", () => {
    renderSettings();
    expect(screen.getByText("Save Defaults")).toBeInTheDocument();
  });

  it("Reset All button is present", () => {
    renderSettings();
    expect(screen.getByText("Reset All")).toBeInTheDocument();
  });

  it("Save Defaults button changes to Saved on click", async () => {
    renderSettings();
    const saveBtn = screen.getByText("Save Defaults");
    await userEvent.click(saveBtn);
    expect(await screen.findByText("Saved")).toBeInTheDocument();
  });

  it("allows changing theme radio selection", async () => {
    renderSettings();
    const lightRadio = screen.getByLabelText("Light");
    await userEvent.click(lightRadio);
    expect(lightRadio).toBeChecked();
  });

  it("shows integration status labels", () => {
    renderSettings();
    const connectedBadges = screen.getAllByText("Connected");
    expect(connectedBadges.length).toBe(3);
  });

  it("renders font size selector", () => {
    renderSettings();
    expect(screen.getByDisplayValue("Medium")).toBeInTheDocument();
  });

  it("clicking a stack card selects it and persists via backend", async () => {
    mockInvoke.mockResolvedValue({
      preferred_stack: "nextjs-supabase-vercel",
      default_deploy_target: "vercel",
      auto_provision: true,
    });
    renderSettings();

    const fastapiCard = await screen.findByText("Next.js + Supabase + FastAPI");
    await userEvent.click(fastapiCard);

    expect(mockInvoke).toHaveBeenLastCalledWith("set_preferences_cmd", {
      preferences: {
        preferred_stack: "nextjs-supabase-fastapi",
        default_deploy_target: "vercel",
        auto_provision: true,
      },
    });
  });

  describe("Why ACC positioning panel", () => {
    it("renders Why ACC section heading", () => {
      renderSettings();
      expect(screen.getByText("Why ACC?")).toBeInTheDocument();
    });

    it("renders uncontested feature items", () => {
      renderSettings();
      expect(screen.getByText(/Dependency-Aware Wave Execution/i)).toBeInTheDocument();
      expect(screen.getByText(/Handoff Verification Gates/i)).toBeInTheDocument();
      expect(screen.getByText(/Proactive Token Budget/i)).toBeInTheDocument();
      expect(screen.getByText(/WIP Checkpoint/i)).toBeInTheDocument();
      expect(screen.getByText(/2-Pass Knowledge Compounding/i)).toBeInTheDocument();
      expect(screen.getByText(/7-Stage Connector Loop/i)).toBeInTheDocument();
      expect(screen.getByText(/Correction Loop/i)).toBeInTheDocument();
      expect(screen.getByText(/SkillBridge Ecosystem/i)).toBeInTheDocument();
    });

    it("renders Tauri v2 size comparison", () => {
      renderSettings();
      expect(screen.getByText(/~10MB/i)).toBeInTheDocument();
      expect(screen.getByText(/150MB\+/i)).toBeInTheDocument();
    });

    it("renders market position summary", () => {
      renderSettings();
      expect(screen.getByText(/agent orchestration/i)).toBeInTheDocument();
      expect(screen.getByText(/knowledge management/i)).toBeInTheDocument();
      expect(screen.getByText(/desktop productivity/i)).toBeInTheDocument();
      expect(screen.getByText(/team collaboration/i)).toBeInTheDocument();
    });

    it("renders competitive analysis footnote", () => {
      renderSettings();
      expect(screen.getByText(/competitive analysis of 13 products/i)).toBeInTheDocument();
    });
  });
});
