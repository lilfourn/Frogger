import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { GalleryView } from "./GalleryView";
import type { FileEntry } from "../../types/file";

function makeEntry(name: string, isDir = false): FileEntry {
  return {
    path: `/${name}`,
    name,
    extension: isDir ? null : (name.split(".").pop() ?? null),
    mime_type: null,
    size_bytes: 2048,
    created_at: null,
    modified_at: "2024-01-15T10:00:00Z",
    is_directory: isDir,
    parent_path: "/",
  };
}

const entries = [
  makeEntry("photo.png"),
  makeEntry("video.mp4"),
  makeEntry("readme.md"),
  makeEntry("docs", true),
];

function defaultProps(overrides: Partial<Parameters<typeof GalleryView>[0]> = {}) {
  return {
    entries,
    onSelect: vi.fn(),
    onOpen: vi.fn(),
    onItemContextMenu: vi.fn(),
    selectedPaths: new Set<string>(),
    ...overrides,
  };
}

describe("GalleryView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders preview area and filmstrip", () => {
    render(<GalleryView {...defaultProps()} />);
    expect(screen.getByTestId("gallery-view")).toBeInTheDocument();
    expect(screen.getByTestId("gallery-preview")).toBeInTheDocument();
    expect(screen.getByTestId("gallery-filmstrip")).toBeInTheDocument();
  });

  it("shows image preview via asset protocol for image files", () => {
    render(<GalleryView {...defaultProps()} />);
    const previewImg = screen.getByTestId("gallery-preview").querySelector("img");
    expect(previewImg).toBeInTheDocument();
    expect(previewImg?.src).toContain("asset://localhost//photo.png");
  });

  it("shows FileIcon for non-image files in preview", () => {
    render(<GalleryView {...defaultProps({ entries: [makeEntry("readme.md")] })} />);
    const preview = screen.getByTestId("gallery-preview");
    const icon = preview.querySelector("img[alt='file']");
    expect(icon).toBeInTheDocument();
  });

  it("shows file name in preview", () => {
    render(<GalleryView {...defaultProps()} />);
    const preview = screen.getByTestId("gallery-preview");
    expect(preview.textContent).toContain("photo.png");
  });

  it("renders filmstrip items with file names", () => {
    render(<GalleryView {...defaultProps()} />);
    const filmstrip = screen.getByTestId("gallery-filmstrip");
    const items = filmstrip.querySelectorAll("[data-testid='filmstrip-item']");
    expect(items.length).toBe(4);

    expect(items[0].textContent).toContain("photo.png");
    expect(items[1].textContent).toContain("video.mp4");
    expect(items[2].textContent).toContain("readme.md");
    expect(items[3].textContent).toContain("docs");
  });

  it("applies carousel scale to filmstrip items based on distance from selected", () => {
    render(<GalleryView {...defaultProps()} />);
    const items = screen.getByTestId("gallery-filmstrip").querySelectorAll("[data-testid='filmstrip-item']");

    expect((items[0] as HTMLElement).style.transform).toBe("scale(1.15)");
    expect((items[1] as HTMLElement).style.transform).toBe("scale(0.85)");
    expect((items[2] as HTMLElement).style.transform).toBe("scale(0.7)");
    expect((items[3] as HTMLElement).style.transform).toBe("scale(0.6)");
  });

  it("clicking filmstrip item updates preview and calls onSelect", () => {
    const onSelect = vi.fn();
    render(<GalleryView {...defaultProps({ onSelect })} />);
    const filmstrip = screen.getByTestId("gallery-filmstrip");
    const items = filmstrip.querySelectorAll("[data-testid='filmstrip-item']");

    fireEvent.click(items[2]); // readme.md
    expect(onSelect).toHaveBeenCalledWith(entries[2]);

    const preview = screen.getByTestId("gallery-preview");
    expect(preview.textContent).toContain("readme.md");
  });

  it("double-clicking filmstrip item calls onOpen", () => {
    const onOpen = vi.fn();
    render(<GalleryView {...defaultProps({ onOpen })} />);
    const items = screen.getByTestId("gallery-filmstrip").querySelectorAll("[data-testid='filmstrip-item']");

    fireEvent.doubleClick(items[0]);
    expect(onOpen).toHaveBeenCalledWith(entries[0]);
  });

  it("ArrowRight navigates to next entry", () => {
    const onSelect = vi.fn();
    render(<GalleryView {...defaultProps({ onSelect })} />);
    const container = screen.getByTestId("gallery-view");

    fireEvent.keyDown(container, { key: "ArrowRight" });
    expect(onSelect).toHaveBeenCalledWith(entries[1]);
  });

  it("ArrowLeft does not go below 0", () => {
    const onSelect = vi.fn();
    render(<GalleryView {...defaultProps({ onSelect })} />);
    const container = screen.getByTestId("gallery-view");

    fireEvent.keyDown(container, { key: "ArrowLeft" });
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("stays within bounds at last entry", () => {
    const onSelect = vi.fn();
    render(<GalleryView {...defaultProps({ onSelect })} />);
    const container = screen.getByTestId("gallery-view");

    // Navigate to last
    fireEvent.keyDown(container, { key: "ArrowRight" }); // index 1
    fireEvent.keyDown(container, { key: "ArrowRight" }); // index 2
    fireEvent.keyDown(container, { key: "ArrowRight" }); // index 3
    fireEvent.keyDown(container, { key: "ArrowRight" }); // should stay at 3

    expect(onSelect).toHaveBeenCalledTimes(3);
  });
});
