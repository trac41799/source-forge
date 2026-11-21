import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FolderPickerInput, FilePickerInput } from "@/components/ui/path-picker";
import { installWebPreviewShim } from "@/lib/ipc/webPreviewShim";
import { useProjectStore } from "@/stores/projectStore";
import { mockInvoke } from "../setup";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (options: unknown = {}) => mockInvoke("plugin:dialog|open", { options }),
}));

describe("PathPicker components", () => {
  beforeEach(() => {
    useProjectStore.setState({
      currentProject: null,
      recentProjects: [],
      recentPaths: [],
    });
  });

  it("FolderPickerInput opens a directory dialog and forwards the selected folder", async () => {
    mockInvoke.mockResolvedValueOnce("D:/workspace/my-app");
    const onChange = vi.fn();

    render(
      <FolderPickerInput
        id="folder-input"
        value=""
        onChange={onChange}
        placeholder="/path/to/project"
        browseLabel="Browse Folder..."
      />
    );

    await userEvent.click(screen.getByRole("button", { name: /browse folder/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("plugin:dialog|open", {
        options: expect.objectContaining({
          directory: true,
          multiple: false,
        }),
      })
    );
    expect(onChange).toHaveBeenCalledWith("D:/workspace/my-app");
  });

  it("FolderPickerInput offers quick-select options from recentPaths and currentProject", async () => {
    useProjectStore.setState({
      currentProject: {
        id: "proj-1",
        path: "D:/projects/active",
        name: "active",
        stack: [],
        active_agents: [],
        active_skills: [],
        active_mcps: [],
        preferred_models: [],
      },
      recentProjects: ["D:/projects/recent-1"],
      recentPaths: ["D:/projects/recent-1"],
    });
    const onChange = vi.fn();

    render(
      <FolderPickerInput
        value=""
        onChange={onChange}
        placeholder="/path/to/project"
      />
    );

    const select = screen.getByRole("combobox", { name: /recent folders/i });
    await userEvent.selectOptions(select, "D:/projects/recent-1");
    expect(onChange).toHaveBeenCalledWith("D:/projects/recent-1");
  });

  it("FilePickerInput opens a file dialog with filters and forwards the selected file", async () => {
    mockInvoke.mockResolvedValueOnce("docs/SPEC.md");
    const onChange = vi.fn();

    render(
      <FilePickerInput
        id="file-input"
        value=""
        onChange={onChange}
        placeholder="docs/PLAN.md"
        browseLabel="Browse Spec..."
        filters={[{ name: "Markdown", extensions: ["md", "markdown"] }]}
      />
    );

    await userEvent.click(screen.getByRole("button", { name: /browse spec/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("plugin:dialog|open", {
        options: expect.objectContaining({
          directory: false,
          multiple: false,
          filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
        }),
      })
    );
    expect(onChange).toHaveBeenCalledWith("docs/SPEC.md");
  });

  it("webPreviewShim handles plugin:dialog|open for directory and file dialogs", async () => {
    const win = window as unknown as Record<string, unknown>;
    const prevInternals = win.__TAURI_INTERNALS__;
    delete win.__TAURI_INTERNALS__;

    try {
      installWebPreviewShim();
      const internals = win.__TAURI_INTERNALS__ as {
        invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
      };

      const folder = await internals.invoke("plugin:dialog|open", {
        options: { directory: true },
      });
      const file = await internals.invoke("plugin:dialog|open", {
        options: { directory: false },
      });

      expect(folder).toBe("D:/projects/demo-app");
      expect(file).toBe("docs/PLAN.md");
    } finally {
      win.__TAURI_INTERNALS__ = prevInternals;
    }
  });
});
