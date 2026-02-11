import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { Sidebar } from "./Sidebar";
import { useFileStore } from "../../stores/fileStore";

vi.mock("../../services/fileService", () => ({
  getHomeDir: vi.fn().mockResolvedValue("/Users/testuser"),
  getMountedVolumes: vi
    .fn()
    .mockResolvedValue([
      { name: "Macintosh HD", path: "/", total_bytes: 500000000000, free_bytes: 200000000000 },
    ]),
}));

describe("Sidebar", () => {
  beforeEach(() => {
    useFileStore.setState(useFileStore.getInitialState());
  });

  it("renders favorites section", async () => {
    render(<Sidebar />);
    await waitFor(() => {
      expect(screen.getByText("Favorites")).toBeInTheDocument();
    });
  });

  it("renders default bookmark items", async () => {
    render(<Sidebar />);
    await waitFor(() => {
      expect(screen.getByText("Home")).toBeInTheDocument();
    });
    expect(screen.getByText("Desktop")).toBeInTheDocument();
    expect(screen.getByText("Documents")).toBeInTheDocument();
    expect(screen.getByText("Downloads")).toBeInTheDocument();
  });

  it("clicking a bookmark navigates to that path", async () => {
    render(<Sidebar />);
    await waitFor(() => {
      expect(screen.getByText("Desktop")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText("Desktop"));
    expect(useFileStore.getState().currentPath).toContain("Desktop");
  });

  it("renders recents section when paths exist", async () => {
    useFileStore.setState({ recentPaths: ["/tmp", "/var"] });
    render(<Sidebar />);
    await waitFor(() => {
      expect(screen.getByText("Recents")).toBeInTheDocument();
    });
    expect(screen.getByText("/tmp")).toBeInTheDocument();
    expect(screen.getByText("/var")).toBeInTheDocument();
  });

  it("does not render recents section when empty", async () => {
    render(<Sidebar />);
    await waitFor(() => {
      expect(screen.getByText("Favorites")).toBeInTheDocument();
    });
    expect(screen.queryByText("Recents")).not.toBeInTheDocument();
  });
});
