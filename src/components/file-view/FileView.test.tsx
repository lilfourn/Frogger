import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { FileView } from "./FileView";
import { useSettingsStore } from "../../stores/settingsStore";
import { useFileStore } from "../../stores/fileStore";
import type { FileEntry } from "../../types/file";

const mockEntries: FileEntry[] = [
  {
    path: "/test/docs",
    name: "docs",
    extension: null,
    mime_type: null,
    size_bytes: null,
    created_at: "2025-01-01T00:00:00Z",
    modified_at: "2025-01-15T00:00:00Z",
    is_directory: true,
    parent_path: "/test",
  },
  {
    path: "/test/readme.md",
    name: "readme.md",
    extension: "md",
    mime_type: "text/markdown",
    size_bytes: 2048,
    created_at: "2025-01-01T00:00:00Z",
    modified_at: "2025-02-01T00:00:00Z",
    is_directory: false,
    parent_path: "/test",
  },
  {
    path: "/test/photo.png",
    name: "photo.png",
    extension: "png",
    mime_type: "image/png",
    size_bytes: 1048576,
    created_at: "2025-01-01T00:00:00Z",
    modified_at: "2025-01-20T00:00:00Z",
    is_directory: false,
    parent_path: "/test",
  },
];

describe("FileView", () => {
  beforeEach(() => {
    useSettingsStore.setState(useSettingsStore.getInitialState());
    useFileStore.setState({ ...useFileStore.getInitialState(), entries: mockEntries });
  });

  it("renders list view by default", () => {
    render(<FileView />);
    expect(screen.getByTestId("list-view")).toBeInTheDocument();
  });

  it("renders grid view when viewMode is grid", () => {
    useSettingsStore.setState({ viewMode: "grid" });
    render(<FileView />);
    expect(screen.getByTestId("grid-view")).toBeInTheDocument();
  });

  it("displays all entries in list view", () => {
    render(<FileView />);
    expect(screen.getByText("docs")).toBeInTheDocument();
    expect(screen.getByText("readme.md")).toBeInTheDocument();
    expect(screen.getByText("photo.png")).toBeInTheDocument();
  });

  it("displays all entries in grid view", () => {
    useSettingsStore.setState({ viewMode: "grid" });
    render(<FileView />);
    expect(screen.getByText("docs")).toBeInTheDocument();
    expect(screen.getByText("readme.md")).toBeInTheDocument();
    expect(screen.getByText("photo.png")).toBeInTheDocument();
  });

  it("renders column view when viewMode is column", () => {
    useSettingsStore.setState({ viewMode: "column" });
    render(<FileView />);
    expect(screen.getByTestId("column-view")).toBeInTheDocument();
  });

  it("renders gallery view when viewMode is gallery", () => {
    useSettingsStore.setState({ viewMode: "gallery" });
    render(<FileView />);
    expect(screen.getByTestId("gallery-view")).toBeInTheDocument();
  });
});
