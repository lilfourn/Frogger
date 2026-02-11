import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TitleBar } from "./TitleBar";
import { useSettingsStore } from "../../stores/settingsStore";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  }),
}));

describe("TitleBar", () => {
  beforeEach(() => {
    useSettingsStore.setState(useSettingsStore.getInitialState());
  });

  it("renders title bar with app name", () => {
    render(<TitleBar />);
    expect(screen.getByTestId("title-bar")).toBeInTheDocument();
    expect(screen.getByText("Frogger")).toBeInTheDocument();
  });

  it("renders window control buttons", () => {
    render(<TitleBar />);
    expect(screen.getByLabelText("Minimize")).toBeInTheDocument();
    expect(screen.getByLabelText("Maximize")).toBeInTheDocument();
    expect(screen.getByLabelText("Close")).toBeInTheDocument();
  });

  it("renders theme toggle button", () => {
    render(<TitleBar />);
    expect(screen.getByLabelText("Toggle theme")).toBeInTheDocument();
  });

  it("cycles theme on toggle click", () => {
    render(<TitleBar />);
    const toggle = screen.getByLabelText("Toggle theme");

    expect(useSettingsStore.getState().theme).toBe("system");
    fireEvent.click(toggle);
    expect(useSettingsStore.getState().theme).toBe("light");
    fireEvent.click(toggle);
    expect(useSettingsStore.getState().theme).toBe("dark");
    fireEvent.click(toggle);
    expect(useSettingsStore.getState().theme).toBe("system");
  });
});
