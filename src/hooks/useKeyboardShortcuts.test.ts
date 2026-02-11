import { describe, it, expect, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { useKeyboardShortcuts } from "./useKeyboardShortcuts";

describe("useKeyboardShortcuts", () => {
  it("fires handler on matching key combo", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ key: "z", meta: true, handler }]),
    );

    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "z", metaKey: true }),
    );
    expect(handler).toHaveBeenCalledOnce();
  });

  it("does not fire for non-matching combo", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ key: "z", meta: true, handler }]),
    );

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "z" }));
    expect(handler).not.toHaveBeenCalled();
  });

  it("supports shift modifier", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([
        { key: "z", meta: true, shift: true, handler },
      ]),
    );

    document.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "z",
        metaKey: true,
        shiftKey: true,
      }),
    );
    expect(handler).toHaveBeenCalledOnce();
  });
});
