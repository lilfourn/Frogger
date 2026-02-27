import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useTauriEvent } from "./useTauriEvents";

const mockUnlisten = vi.fn();
const mockListen = vi.fn().mockResolvedValue(mockUnlisten);

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

describe("useTauriEvent", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("listens to specified event", () => {
    const handler = vi.fn();
    renderHook(() => useTauriEvent("test-event", handler));
    expect(mockListen).toHaveBeenCalledWith("test-event", expect.any(Function));
  });

  it("calls handler when event fires", async () => {
    const handler = vi.fn();
    let capturedCallback: (event: { payload: unknown }) => void = () => {};
    mockListen.mockImplementation((_name: string, cb: (event: { payload: unknown }) => void) => {
      capturedCallback = cb;
      return Promise.resolve(mockUnlisten);
    });

    renderHook(() => useTauriEvent("test-event", handler));

    act(() => {
      capturedCallback({ payload: { progress: 50 } });
    });

    expect(handler).toHaveBeenCalledWith({ progress: 50 });
  });

  it("unlistens on unmount", async () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() => useTauriEvent("test-event", handler));

    await vi.waitFor(() => {
      expect(mockListen).toHaveBeenCalled();
    });

    unmount();

    await vi.waitFor(() => {
      expect(mockUnlisten).toHaveBeenCalled();
    });
  });

  it("cleans up listener when listen resolves after unmount", async () => {
    const handler = vi.fn();
    let resolveListen: ((fn: () => void) => void) | undefined;
    mockListen.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveListen = (fn: () => void) => resolve(fn);
        }),
    );

    const delayedUnlisten = vi.fn();
    const { unmount } = renderHook(() => useTauriEvent("test-event", handler));
    unmount();

    if (typeof resolveListen !== "function") {
      throw new Error("listen resolver was not set");
    }
    resolveListen(delayedUnlisten);

    await vi.waitFor(() => {
      expect(delayedUnlisten).toHaveBeenCalledTimes(1);
    });
  });
});
