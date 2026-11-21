import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import App from "@/App";
import { Sidebar } from "@/components/layout/Sidebar";
import { ThemeProvider } from "@/components/ThemeProvider";
import { mockInvoke } from "@/__tests__/setup";
import { useProjectStore } from "@/stores/projectStore";
import { open } from "@tauri-apps/plugin-dialog";
import Integrations from "@/pages/Integrations";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

describe("Integrations page", () => {
  it("renders at /integrations route", () => {
    render(
      <MemoryRouter initialEntries={["/integrations"]}>
        <App />
      </MemoryRouter>
    );
    const elements = screen.getAllByText(/Integrations/);
    expect(elements.length).toBeGreaterThanOrEqual(1);
  });

  it("redirects /connectors to /integrations", () => {
    render(
      <MemoryRouter initialEntries={["/connectors"]}>
        <App />
      </MemoryRouter>
    );
    const elements = screen.getAllByText(/Integrations/);
    expect(elements.length).toBeGreaterThanOrEqual(1);
  });

  it("sidebar shows Integrations label in CONFIGURE group", () => {
    render(
      <MemoryRouter>
        <ThemeProvider>
          <Sidebar />
        </ThemeProvider>
      </MemoryRouter>
    );
    expect(screen.getByRole("link", { name: /Integrations/ })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: /Connectors/ })).not.toBeInTheDocument();
  });

  it("auto-defaults projectPath from active project and picks folder via Browse Folder", async () => {
    mockInvoke.mockResolvedValue([]);
    useProjectStore.setState({
      currentProject: {
        id: "p1",
        path: "/repo/demo",
        name: "demo",
        stack: [],
        active_agents: [],
        active_skills: [],
        active_mcps: [],
        preferred_models: [],
      },
    });
    const mockOpen = vi.mocked(open);
    mockOpen.mockReset();

    render(
      <MemoryRouter>
        <ThemeProvider>
          <Integrations />
        </ThemeProvider>
      </MemoryRouter>
    );

    const projectInput = screen.getByPlaceholderText("/path/to/project");
    expect(projectInput).toHaveValue("/repo/demo");

    mockOpen.mockResolvedValueOnce("/picked/repo");
    await userEvent.click(screen.getByRole("button", { name: /Browse Folder/i }));
    expect(mockOpen).toHaveBeenCalledWith(expect.objectContaining({ directory: true }));
    expect(projectInput).toHaveValue("/picked/repo");
    useProjectStore.setState({ currentProject: null });
  });

  it("picks daemon config file via Browse Config in Chat tab", async () => {
    mockInvoke.mockResolvedValue([]);
    const mockOpen = vi.mocked(open);
    mockOpen.mockReset();

    render(
      <MemoryRouter>
        <ThemeProvider>
          <Integrations />
        </ThemeProvider>
      </MemoryRouter>
    );

    await userEvent.click(screen.getByRole("button", { name: /^Chat$/i }));
    mockOpen.mockResolvedValueOnce("/etc/daemon/config.yaml");
    await userEvent.click(screen.getByRole("button", { name: /Browse Config/i }));
    expect(mockOpen).toHaveBeenCalledWith(
      expect.objectContaining({
        directory: false,
        filters: [{ name: "YAML", extensions: ["yaml", "yml", "json"] }],
      })
    );
    expect(screen.getByPlaceholderText("/path/to/daemon/config.yaml")).toHaveValue(
      "/etc/daemon/config.yaml"
    );
  });
});
