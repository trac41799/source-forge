import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import BuildApp from "@/pages/BuildApp";
import { mockInvoke } from "../setup";

const REPORT = {
  run_id: "run-1",
  status: "succeeded",
  stack_id: "nextjs-prisma-vercel",
  stages: [
    { name: "parse_spec", status: "done", message: "1 tasks parsed" },
    { name: "deploy", status: "done", message: "deployed via mock" },
  ],
  verification: {
    passed: false,
    checks: [
      {
        name: "SPA rewrite excludes /api",
        status: { Fail: "missing" },
        detail: "",
      },
    ],
  },
  deploy: { provider: "mock", url: "https://mock.example.app", detail: "" },
  compounder_items: 2,
  error: null,
};

describe("BuildApp page", () => {
  it("renders the build form", () => {
    render(<BuildApp />);
    expect(
      screen.getByRole("heading", { name: "Build App" })
    ).toBeInTheDocument();
    expect(screen.getByPlaceholderText("docs/PLAN.md")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /build app/i })).toBeInTheDocument();
  });

  it("starts a build with pipeline options and renders the report", async () => {
    mockInvoke.mockResolvedValueOnce(REPORT);
    render(<BuildApp />);

    await userEvent.type(
      screen.getByPlaceholderText("docs/PLAN.md"),
      "docs/PLAN.md"
    );
    await userEvent.type(
      screen.getByPlaceholderText("/path/to/project"),
      "/tmp/app"
    );
    await userEvent.click(screen.getByRole("button", { name: /build app/i }));

    await screen.findByText("succeeded");
    expect(mockInvoke).toHaveBeenCalledWith("build_app_cmd", {
      options: expect.objectContaining({
        spec_path: "docs/PLAN.md",
        project_path: "/tmp/app",
        stack_id: "nextjs-supabase-vercel",
        agent_command: "opencode",
        generate_dockerfile: true,
      }),
    });
    expect(screen.getByText(/mock.example.app/)).toBeInTheDocument();
    expect(screen.getByText(/SPA rewrite excludes \/api/)).toBeInTheDocument();
    expect(screen.getByText(/2 knowledge items compounded/)).toBeInTheDocument();
  });

  it("offers Cancel for a running build and cancels the active run", async () => {
    // `build_app_cmd` only returns when the pipeline finishes, so the page polls
    // the active run to learn its id; that is what makes Cancel possible.
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "build_app_cmd") return new Promise(() => {});
      if (cmd === "get_active_build_run_cmd") return { id: "run-42" };
      if (cmd === "cancel_build_app_cmd") return true;
      return undefined;
    });

    render(<BuildApp />);
    await userEvent.type(screen.getByPlaceholderText("docs/PLAN.md"), "docs/PLAN.md");
    await userEvent.type(screen.getByPlaceholderText("/path/to/project"), "/tmp/app");
    await userEvent.click(screen.getByRole("button", { name: /build app/i }));

    const cancel = await screen.findByRole("button", { name: /cancel/i });
    await waitFor(() => expect(cancel).toBeEnabled());
    await userEvent.click(cancel);

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("cancel_build_app_cmd", {
        runId: "run-42",
      })
    );
  });
});
