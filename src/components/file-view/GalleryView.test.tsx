import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { GalleryView } from "./GalleryView";
import type { FileEntry } from "../../types/file";

function makeEntry(name: string, isDir = false): FileEntry {
  return {
    path: `/${name}`,
    name,
    extension: isDir ? null : name.split(".").pop() ?? null,
    mime_type: null,
    size_bytes: 100,
    created_at: null,
    modified_at: null,
    is_directory: isDir,
    parent_path: "/",
  };
}

describe("GalleryView", () => {
  const entries = [makeEntry("photo.png"), makeEntry("docs", true), makeEntry("readme.md")];

  it("renders gallery grid", () => {
    render(<GalleryView entries={entries} onNavigate={vi.fn()} />);
    expect(screen.getByTestId("gallery-view")).toBeInTheDocument();
  });

  it("renders file names", () => {
    render(<GalleryView entries={entries} onNavigate={vi.fn()} />);
    expect(screen.getByText("photo.png")).toBeInTheDocument();
    expect(screen.getByText("docs")).toBeInTheDocument();
    expect(screen.getByText("readme.md")).toBeInTheDocument();
  });

  it("shows large icons", () => {
    render(<GalleryView entries={entries} onNavigate={vi.fn()} />);
    const images = screen.getAllByRole("img");
    expect(images.length).toBeGreaterThanOrEqual(3);
    images.forEach((img) => {
      expect(img).toHaveAttribute("width", "80");
    });
  });
});
