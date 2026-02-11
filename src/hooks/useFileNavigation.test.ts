import { describe, it, expect, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useFileNavigation, resetFileNavigation } from "./useFileNavigation";
import type { FileEntry } from "../types/file";

function makeEntry(name: string, isDir = false): FileEntry {
  return {
    path: `/${name}`,
    name,
    extension: isDir ? null : "txt",
    mime_type: null,
    size_bytes: 100,
    created_at: null,
    modified_at: null,
    is_directory: isDir,
    parent_path: "/",
  };
}

describe("useFileNavigation", () => {
  const entries = [makeEntry("dir1", true), makeEntry("a.txt"), makeEntry("b.txt")];

  beforeEach(() => {
    resetFileNavigation();
  });

  it("starts with focusIndex -1", () => {
    const { result } = renderHook(() => useFileNavigation(entries));
    expect(result.current.focusIndex).toBe(-1);
  });

  it("moveDown advances focus", () => {
    const { result } = renderHook(() => useFileNavigation(entries));
    act(() => result.current.moveDown());
    expect(result.current.focusIndex).toBe(0);
    act(() => result.current.moveDown());
    expect(result.current.focusIndex).toBe(1);
  });

  it("moveUp decreases focus", () => {
    const { result } = renderHook(() => useFileNavigation(entries));
    act(() => result.current.setFocusIndex(2));
    act(() => result.current.moveUp());
    expect(result.current.focusIndex).toBe(1);
  });

  it("moveDown does not exceed entries length", () => {
    const { result } = renderHook(() => useFileNavigation(entries));
    act(() => result.current.setFocusIndex(2));
    act(() => result.current.moveDown());
    expect(result.current.focusIndex).toBe(2);
  });

  it("moveUp does not go below 0", () => {
    const { result } = renderHook(() => useFileNavigation(entries));
    act(() => result.current.setFocusIndex(0));
    act(() => result.current.moveUp());
    expect(result.current.focusIndex).toBe(0);
  });

  it("focusedEntry returns correct entry", () => {
    const { result } = renderHook(() => useFileNavigation(entries));
    act(() => result.current.setFocusIndex(1));
    expect(result.current.focusedEntry).toBe(entries[1]);
  });

  it("resets focusIndex when entries change", () => {
    const { result, rerender } = renderHook(({ e }) => useFileNavigation(e), {
      initialProps: { e: entries },
    });
    act(() => result.current.setFocusIndex(2));
    rerender({ e: [makeEntry("new.txt")] });
    expect(result.current.focusIndex).toBe(-1);
  });
});
