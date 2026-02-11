import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QuickLookPanel } from "./QuickLookPanel";

describe("QuickLookPanel", () => {
  it("renders nothing when not open", () => {
    const { container } = render(
      <QuickLookPanel isOpen={false} filePath={null} previewType="unknown" onClose={vi.fn()} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders overlay when open", () => {
    render(
      <QuickLookPanel
        isOpen={true}
        filePath="/test/image.png"
        previewType="image"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByTestId("quick-look-overlay")).toBeInTheDocument();
  });

  it("shows file name in header", () => {
    render(
      <QuickLookPanel
        isOpen={true}
        filePath="/test/image.png"
        previewType="image"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText("image.png")).toBeInTheDocument();
  });

  it("calls onClose when close button clicked", () => {
    const onClose = vi.fn();
    render(
      <QuickLookPanel
        isOpen={true}
        filePath="/test/image.png"
        previewType="image"
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText("Close preview"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("calls onClose when Escape pressed", () => {
    const onClose = vi.fn();
    render(
      <QuickLookPanel
        isOpen={true}
        filePath="/test/image.png"
        previewType="image"
        onClose={onClose}
      />,
    );
    fireEvent.keyDown(screen.getByTestId("quick-look-overlay"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("shows unsupported message for unknown types", () => {
    render(
      <QuickLookPanel
        isOpen={true}
        filePath="/test/data.bin"
        previewType="unknown"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText(/no preview available/i)).toBeInTheDocument();
  });
});
