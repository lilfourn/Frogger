import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { AppLayout } from "./AppLayout";
import { useSettingsStore } from "../../stores/settingsStore";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({ processed: 0, total: 0, status: "done" }),
}));

describe("AppLayout", () => {
  beforeEach(() => {
    useSettingsStore.setState(useSettingsStore.getInitialState());
  });

  it("renders sidebar, main panel, and status bar", () => {
    render(<AppLayout sidebar={<div>Sidebar Content</div>} main={<div>Main Content</div>} />);

    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
    expect(screen.getByTestId("main-panel")).toBeInTheDocument();
    expect(screen.getByTestId("status-bar")).toBeInTheDocument();
  });

  it("renders sidebar content and main content", () => {
    render(<AppLayout sidebar={<div>Sidebar Content</div>} main={<div>Main Content</div>} />);

    expect(screen.getByText("Sidebar Content")).toBeInTheDocument();
    expect(screen.getByText("Main Content")).toBeInTheDocument();
  });

  it("hides sidebar when sidebarVisible is false", () => {
    useSettingsStore.setState({ sidebarVisible: false });

    render(<AppLayout sidebar={<div>Sidebar Content</div>} main={<div>Main Content</div>} />);

    expect(screen.queryByTestId("sidebar")).not.toBeInTheDocument();
  });
});
