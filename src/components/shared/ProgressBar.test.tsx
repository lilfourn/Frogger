import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ProgressBar } from "./ProgressBar";

describe("ProgressBar", () => {
  it("renders progress percentage", () => {
    render(
      <ProgressBar
        label="Copying files..."
        progress={45}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByText("45%")).toBeInTheDocument();
    expect(screen.getByText("Copying files...")).toBeInTheDocument();
  });

  it("renders progress bar at correct width", () => {
    render(
      <ProgressBar
        label="Copying files..."
        progress={60}
        onCancel={vi.fn()}
      />,
    );
    const bar = screen.getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuenow", "60");
  });

  it("calls onCancel when cancel button clicked", () => {
    const onCancel = vi.fn();
    render(
      <ProgressBar label="Copying files..." progress={30} onCancel={onCancel} />,
    );
    fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("clamps progress between 0 and 100", () => {
    render(
      <ProgressBar label="test" progress={150} onCancel={vi.fn()} />,
    );
    const bar = screen.getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuenow", "100");
  });

  it("does not render when not visible", () => {
    const { container } = render(
      <ProgressBar label="test" progress={50} onCancel={vi.fn()} visible={false} />,
    );
    expect(container.firstChild).toBeNull();
  });
});
