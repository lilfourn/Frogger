import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Toolbar } from "./Toolbar";
import { useSettingsStore } from "../../stores/settingsStore";
import { useFileStore } from "../../stores/fileStore";

describe("Toolbar", () => {
  beforeEach(() => {
    useSettingsStore.setState(useSettingsStore.getInitialState());
    useFileStore.setState(useFileStore.getInitialState());
  });

  it("renders view toggle buttons", () => {
    render(<Toolbar />);
    expect(screen.getByLabelText("List view")).toBeInTheDocument();
    expect(screen.getByLabelText("Grid view")).toBeInTheDocument();
  });

  it("clicking grid toggle switches to grid view", () => {
    render(<Toolbar />);
    fireEvent.click(screen.getByLabelText("Grid view"));
    expect(useSettingsStore.getState().viewMode).toBe("grid");
  });

  it("clicking list toggle switches to list view", () => {
    useSettingsStore.setState({ viewMode: "grid" });
    render(<Toolbar />);
    fireEvent.click(screen.getByLabelText("List view"));
    expect(useSettingsStore.getState().viewMode).toBe("list");
  });

  it("renders path breadcrumb", () => {
    useFileStore.setState({ currentPath: "/Users/test/Documents" });
    render(<Toolbar />);
    expect(screen.getByText("Documents")).toBeInTheDocument();
    expect(screen.getByText("test")).toBeInTheDocument();
  });

  it("clicking path segment navigates", () => {
    useFileStore.setState({ currentPath: "/Users/test/Documents" });
    render(<Toolbar />);
    fireEvent.click(screen.getByText("Users"));
    expect(useFileStore.getState().currentPath).toBe("/Users");
  });
});
