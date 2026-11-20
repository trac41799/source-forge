import { render, act } from "@testing-library/react";
import PtyTerminal from "@/components/terminal/PtyTerminal";
import { mockInvoke } from "../setup";

const mockWriteln = vi.fn();
const mockWrite = vi.fn();
const mockClear = vi.fn();
const mockFocus = vi.fn();
const mockDispose = vi.fn();
const mockOpen = vi.fn();
const mockLoadAddon = vi.fn();
const mockFit = vi.fn();

let capturedOnData: ((data: string) => void) | null = null;
const mockOnData = vi.fn((cb: (data: string) => void) => {
  capturedOnData = cb;
  return { dispose: vi.fn() };
});

vi.mock("xterm", () => ({
  Terminal: class {
    open = mockOpen;
    loadAddon = mockLoadAddon;
    writeln = mockWriteln;
    write = mockWrite;
    clear = mockClear;
    focus = mockFocus;
    dispose = mockDispose;
    onData = mockOnData;
    cols = 120;
    rows = 36;
  },
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit = mockFit;
  },
}));

type EventCallback = (event: {
  payload: { agent_id: string; session_id: string; line: string; timestamp: string };
}) => void;

const listeners = new Map<string, EventCallback>();
const mockUnlisten = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, cb: EventCallback) => {
    listeners.set(eventName, cb);
    return Promise.resolve(mockUnlisten);
  }),
  emit: vi.fn().mockResolvedValue(undefined),
}));

describe("PtyTerminal", () => {
  beforeEach(() => {
    listeners.clear();
    capturedOnData = null;
    mockWriteln.mockClear();
    mockWrite.mockClear();
    mockOnData.mockClear();
    mockInvoke.mockResolvedValue(undefined);
  });

  it("streams_output_and_forwards_input", async () => {
    render(<PtyTerminal agentId="claude" />);

    // Wait for async listen registration
    await act(async () => {
      await Promise.resolve();
    });

    const outputListener = listeners.get("agent-output");
    expect(outputListener).toBeDefined();

    // Emit matching agent-output event
    act(() => {
      outputListener!({
        payload: {
          agent_id: "claude",
          session_id: "sess-1",
          line: "Welcome to Claude Code",
          timestamp: "2026-11-20T00:00:00Z",
        },
      });
      // Non-matching agent_id should be ignored
      outputListener!({
        payload: {
          agent_id: "opencode",
          session_id: "sess-2",
          line: "Should not appear",
          timestamp: "2026-11-20T00:00:00Z",
        },
      });
    });

    expect(mockWriteln).toHaveBeenCalledTimes(1);
    expect(mockWriteln).toHaveBeenCalledWith("Welcome to Claude Code");

    // Forward terminal.onData input to write_to_agent
    expect(capturedOnData).not.toBeNull();
    await act(async () => {
      capturedOnData!("help\r");
      await Promise.resolve();
    });

    expect(mockInvoke).toHaveBeenCalledWith("write_to_agent", {
      agentId: "claude",
      text: "help\r",
    });
  });
});
