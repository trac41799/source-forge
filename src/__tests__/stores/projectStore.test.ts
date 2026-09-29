import { vi } from "vitest";
import { useProjectStore } from "@/stores/projectStore";
import { detectStack as detectStackFromFs } from "@/lib/project/detector";

// projectStore.detectStack now delegates to the local TS detector
// (src/lib/project/detector.ts) instead of the removed `detect_stack`
// backend command.
vi.mock("@/lib/project/detector", () => ({
  detectStack: vi.fn(),
}));

const mockDetect = detectStackFromFs as unknown as ReturnType<typeof vi.fn>;

beforeEach(() => {
  mockDetect.mockReset();
  useProjectStore.setState({
    currentProject: null,
    recentProjects: [],
    recentPaths: [],
  });
});

describe("projectStore", () => {
  it("starts with null currentProject", () => {
    expect(useProjectStore.getState().currentProject).toBeNull();
  });

  it("detectStack merges the detected partial into a full profile", async () => {
    mockDetect.mockResolvedValueOnce({ stack: ["react", "typescript"] });
    const result = await useProjectStore.getState().detectStack("/test/proj");
    expect(mockDetect).toHaveBeenCalledWith("/test/proj");
    expect(result.path).toBe("/test/proj");
    expect(result.name).toBe("proj");
    expect(result.stack).toEqual(["react", "typescript"]);
  });

  it("detectStack returns fallback when detection fails", async () => {
    mockDetect.mockRejectedValueOnce(new Error("not found"));
    const result = await useProjectStore.getState().detectStack("/unknown");
    expect(result.path).toBe("/unknown");
    expect(result.stack).toEqual([]);
    expect(result.name).toBe("unknown");
  });

  it("switchProject sets currentProject and adds to recent", async () => {
    mockDetect.mockResolvedValueOnce({ stack: ["rust"] });
    await useProjectStore.getState().switchProject("/test/proj2");
    const state = useProjectStore.getState();
    expect(state.currentProject?.path).toBe("/test/proj2");
    expect(state.currentProject?.stack).toEqual(["rust"]);
    expect(state.recentProjects).toContain("/test/proj2");
  });

  it("switchProject caps recentProjects at 10", async () => {
    const paths = Array.from({ length: 10 }, (_, i) => `/path/${i}`);
    useProjectStore.setState({ recentProjects: paths, recentPaths: paths });
    mockDetect.mockResolvedValueOnce({ stack: [] });
    await useProjectStore.getState().switchProject("/path/new");
    expect(useProjectStore.getState().recentProjects.length).toBe(10);
    expect(useProjectStore.getState().recentProjects[0]).toBe("/path/new");
  });

  it("switchProject does not duplicate recent entries", async () => {
    useProjectStore.setState({
      recentProjects: ["/test/proj2"],
      recentPaths: ["/test/proj2"],
    });
    mockDetect.mockResolvedValueOnce({ stack: ["rust"] });
    await useProjectStore.getState().switchProject("/test/proj2");
    expect(useProjectStore.getState().recentProjects.length).toBe(1);
  });
});
