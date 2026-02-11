import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Breadcrumb } from "./Breadcrumb";
import { useFileStore } from "../../stores/fileStore";

describe("Breadcrumb", () => {
  beforeEach(() => {
    useFileStore.setState({
      ...useFileStore.getInitialState(),
      currentPath: "/Users/test/Documents",
    });
  });

  it("renders path segments", () => {
    render(<Breadcrumb />);
    expect(screen.getByRole("button", { name: "/" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Users" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "test" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Documents" })).toBeInTheDocument();
  });

  it("clicking a segment navigates to that path", () => {
    render(<Breadcrumb />);
    fireEvent.click(screen.getByRole("button", { name: "Users" }));
    expect(useFileStore.getState().currentPath).toBe("/Users");
  });

  it("clicking root segment navigates to /", () => {
    render(<Breadcrumb />);
    fireEvent.click(screen.getByRole("button", { name: "/" }));
    expect(useFileStore.getState().currentPath).toBe("/");
  });

  it("enters edit mode on double click", () => {
    render(<Breadcrumb />);
    fireEvent.doubleClick(screen.getByTestId("breadcrumb"));
    expect(screen.getByRole("textbox")).toBeInTheDocument();
    expect(screen.getByRole("textbox")).toHaveValue("/Users/test/Documents");
  });

  it("submitting edit navigates to typed path", () => {
    render(<Breadcrumb />);
    fireEvent.doubleClick(screen.getByTestId("breadcrumb"));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "/tmp" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(useFileStore.getState().currentPath).toBe("/tmp");
  });

  it("pressing Escape cancels edit mode", () => {
    render(<Breadcrumb />);
    fireEvent.doubleClick(screen.getByTestId("breadcrumb"));
    const input = screen.getByRole("textbox");
    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(useFileStore.getState().currentPath).toBe("/Users/test/Documents");
  });
});
