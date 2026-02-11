import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useDragAndDrop } from "./useDragAndDrop";

describe("useDragAndDrop", () => {
  const onDrop = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("returns drag event handlers", () => {
    const { result } = renderHook(() => useDragAndDrop({ onDrop }));
    expect(result.current.dragHandlers).toBeDefined();
    expect(result.current.dropHandlers).toBeDefined();
    expect(result.current.isDragging).toBe(false);
    expect(result.current.isOver).toBe(false);
  });

  it("sets isDragging on dragStart", () => {
    const { result } = renderHook(() => useDragAndDrop({ onDrop }));
    act(() => {
      result.current.dragHandlers.onDragStart({
        dataTransfer: { setData: vi.fn(), getData: () => "" },
      } as unknown as React.DragEvent);
    });
    expect(result.current.isDragging).toBe(true);
  });

  it("sets isOver on dragEnter and clears on dragLeave", () => {
    const { result } = renderHook(() => useDragAndDrop({ onDrop }));

    act(() => {
      result.current.dropHandlers.onDragEnter({
        preventDefault: vi.fn(),
      } as unknown as React.DragEvent);
    });
    expect(result.current.isOver).toBe(true);

    act(() => {
      result.current.dropHandlers.onDragLeave({} as React.DragEvent);
    });
    expect(result.current.isOver).toBe(false);
  });

  it("calls onDrop with transferred paths on drop", () => {
    const { result } = renderHook(() => useDragAndDrop({ onDrop }));

    act(() => {
      result.current.dropHandlers.onDrop({
        preventDefault: vi.fn(),
        dataTransfer: {
          getData: () => JSON.stringify(["/Users/file.txt"]),
        },
      } as unknown as React.DragEvent);
    });

    expect(onDrop).toHaveBeenCalledWith(["/Users/file.txt"]);
    expect(result.current.isOver).toBe(false);
  });
});
