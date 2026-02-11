import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TitleBar } from "./TitleBar";
import { useSettingsStore } from "../../stores/settingsStore";

describe("TitleBar", () => {
  beforeEach(() => {
    useSettingsStore.setState(useSettingsStore.getInitialState());
  });

  it("renders title bar", () => {
    render(<TitleBar />);
    expect(screen.getByTestId("title-bar")).toBeInTheDocument();
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
