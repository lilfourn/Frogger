import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TabBar } from "./TabBar";
import { useFileStore } from "../../stores/fileStore";

describe("TabBar", () => {
  beforeEach(() => {
    useFileStore.setState(useFileStore.getInitialState());
  });

  it("renders the active tab", () => {
    useFileStore.getState().navigateTo("/Users");
    render(<TabBar />);
    expect(screen.getByText("Users")).toBeInTheDocument();
  });

  it("renders multiple tabs", () => {
    useFileStore.getState().navigateTo("/Users");
    useFileStore.getState().addTab();
    useFileStore.getState().navigateTo("/tmp");
    render(<TabBar />);
    expect(screen.getByText("Users")).toBeInTheDocument();
    expect(screen.getByText("tmp")).toBeInTheDocument();
  });

  it("clicking a tab switches to it", () => {
    useFileStore.getState().navigateTo("/Users");
    useFileStore.getState().addTab();
    useFileStore.getState().navigateTo("/tmp");
    render(<TabBar />);

    fireEvent.click(screen.getByText("Users"));
    expect(useFileStore.getState().currentPath).toBe("/Users");
  });

  it("clicking close removes the tab", () => {
    useFileStore.getState().navigateTo("/Users");
    useFileStore.getState().addTab();
    useFileStore.getState().navigateTo("/tmp");
    render(<TabBar />);

    const closeBtns = screen.getAllByLabelText("Close tab");
    expect(closeBtns).toHaveLength(2);

    fireEvent.click(closeBtns[1]);
    expect(useFileStore.getState().tabs).toHaveLength(1);
  });

  it("new tab button adds a tab", () => {
    useFileStore.getState().navigateTo("/Users");
    render(<TabBar />);

    fireEvent.click(screen.getByLabelText("New tab"));
    expect(useFileStore.getState().tabs).toHaveLength(2);
  });
});
