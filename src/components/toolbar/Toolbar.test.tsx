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

  it("renders sort dropdown", () => {
    render(<Toolbar />);
    expect(screen.getByLabelText("Sort by")).toBeInTheDocument();
  });

  it("changing sort updates fileStore", () => {
    render(<Toolbar />);
    fireEvent.change(screen.getByLabelText("Sort by"), { target: { value: "size" } });
    expect(useFileStore.getState().sortBy).toBe("size");
  });

  it("renders sort direction toggle", () => {
    render(<Toolbar />);
    expect(screen.getByLabelText("Toggle sort direction")).toBeInTheDocument();
  });

  it("clicking sort direction toggles asc/desc", () => {
    render(<Toolbar />);
    expect(useFileStore.getState().sortDirection).toBe("asc");
    fireEvent.click(screen.getByLabelText("Toggle sort direction"));
    expect(useFileStore.getState().sortDirection).toBe("desc");
  });
});
