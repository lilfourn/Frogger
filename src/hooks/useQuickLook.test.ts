import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useQuickLook } from "./useQuickLook";

describe("useQuickLook", () => {
  it("starts closed", () => {
    const { result } = renderHook(() => useQuickLook());
    expect(result.current.isOpen).toBe(false);
    expect(result.current.filePath).toBeNull();
  });

  it("open sets filePath and isOpen", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.open("/test/image.png"));
    expect(result.current.isOpen).toBe(true);
    expect(result.current.filePath).toBe("/test/image.png");
  });

  it("close resets state", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.open("/test/image.png"));
    act(() => result.current.close());
    expect(result.current.isOpen).toBe(false);
    expect(result.current.filePath).toBeNull();
  });

  it("toggle opens and closes", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.toggle("/test/file.txt"));
    expect(result.current.isOpen).toBe(true);
    act(() => result.current.toggle("/test/file.txt"));
    expect(result.current.isOpen).toBe(false);
  });

  it("detects image type", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.open("/test/photo.png"));
    expect(result.current.previewType).toBe("image");
  });

  it("detects code type", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.open("/test/app.tsx"));
    expect(result.current.previewType).toBe("code");
  });

  it("detects markdown type", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.open("/test/readme.md"));
    expect(result.current.previewType).toBe("markdown");
  });

  it("detects video type", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.open("/test/clip.mp4"));
    expect(result.current.previewType).toBe("video");
  });

  it("detects pdf type", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.open("/test/doc.pdf"));
    expect(result.current.previewType).toBe("pdf");
  });

  it("returns unknown for unsupported types", () => {
    const { result } = renderHook(() => useQuickLook());
    act(() => result.current.open("/test/data.bin"));
    expect(result.current.previewType).toBe("unknown");
  });
});
