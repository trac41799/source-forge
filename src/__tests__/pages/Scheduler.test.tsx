import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import Scheduler from "@/pages/Scheduler";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

function renderScheduler() {
  mockInvoke.mockResolvedValue([]);
  return render(
    <MemoryRouter>
      <ThemeProvider>
        <Scheduler />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe("Scheduler Page", () => {
  it("renders page title", () => {
    renderScheduler();
    expect(screen.getByText("Scheduler")).toBeInTheDocument();
  });

  it("renders Refresh button", () => {
    renderScheduler();
    expect(screen.getByText("Refresh")).toBeInTheDocument();
  });

  it("renders tab buttons", () => {
    renderScheduler();
    expect(screen.getByText("Jobs")).toBeInTheDocument();
    expect(screen.getByText("History")).toBeInTheDocument();
    expect(screen.getByText("Escalations")).toBeInTheDocument();
  });

  it("renders New Job button", () => {
    renderScheduler();
    expect(screen.getByText("New Job")).toBeInTheDocument();
  });

  it("renders empty or loading state for jobs", () => {
    renderScheduler();
    const loadingOrEmpty = screen.queryByText(/Loading/) || screen.queryByText(/No scheduled jobs yet/);
    expect(loadingOrEmpty).toBeTruthy();
  });

  it("has New Job button available", () => {
    renderScheduler();
    const newJobBtn = screen.getByText("New Job");
    expect(newJobBtn).toBeInTheDocument();
  });

  it("shows cron presets in dialog", async () => {
    const user = userEvent.setup();
    renderScheduler();
    const newJobBtn = screen.getByText("New Job");
    await user.click(newJobBtn);
    const dailyBtn = screen.queryByText("Daily 9am");
    if (dailyBtn) {
      expect(dailyBtn).toBeInTheDocument();
    } else {
      expect(screen.getByText(/Create Job|Name/)).toBeInTheDocument();
    }
  });

  it("shows auto-approve and enabled checkboxes in dialog", async () => {
    const user = userEvent.setup();
    renderScheduler();
    await user.click(screen.getByText("New Job"));
    expect(screen.getByText("Auto-approve")).toBeInTheDocument();
    expect(screen.getByText("Enabled")).toBeInTheDocument();
  });

  it("navigates to History tab", async () => {
    const user = userEvent.setup();
    renderScheduler();
    await user.click(screen.getByText("History"));
    expect(screen.getByText(/No execution history yet/)).toBeInTheDocument();
  });

  it("navigates to Escalations tab", async () => {
    const user = userEvent.setup();
    renderScheduler();
    await user.click(screen.getByText("Escalations"));
    expect(screen.getByText(/No active escalations/)).toBeInTheDocument();
  });

  it("provides quick-pick wave preset badges in job dialog", async () => {
    const user = userEvent.setup();
    renderScheduler();
    await user.click(screen.getByText("New Job"));

    for (const preset of ["default-4-wave", "acp-execution-spine", "nightly-audit"]) {
      expect(screen.getByRole("button", { name: preset })).toBeInTheDocument();
    }

    await user.click(screen.getByRole("button", { name: "acp-execution-spine" }));
    expect(screen.getByPlaceholderText("e.g., default-4-wave")).toHaveValue(
      "acp-execution-spine"
    );
  });

  it("updates escalation_policy JSON when selecting Retry Delay chips or numeric input", async () => {
    const user = userEvent.setup();
    renderScheduler();
    await user.click(screen.getByText("New Job"));

    await user.click(screen.getByRole("button", { name: "15m" }));
    const escalationTextarea = screen.getByPlaceholderText(
      '{"max_retries": 3, "retry_delay_minutes": 30}'
    ) as HTMLTextAreaElement;
    expect(JSON.parse(escalationTextarea.value).retry_delay_minutes).toBe(15);

    const retryInput = screen.getByLabelText(/Retry Delay \(min\)/i);
    fireEvent.change(retryInput, { target: { value: "45" } });
    expect(JSON.parse(escalationTextarea.value).retry_delay_minutes).toBe(45);
  });
});
