import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ColumnView } from "./ColumnView";
import type { FileEntry } from "../../types/file";

function makeEntry(name: string, isDir = false): FileEntry {
  return {
    path: `/${name}`,
    name,
    extension: isDir ? null : "txt",
    mime_type: null,
    size_bytes: 100,
    created_at: null,
    modified_at: null,
    is_directory: isDir,
    parent_path: "/",
  };
}

describe("ColumnView", () => {
  const entries = [makeEntry("docs", true), makeEntry("file.txt")];

  it("renders entries in a column", () => {
    render(<ColumnView entries={entries} onNavigate={vi.fn()} />);
    expect(screen.getByTestId("column-view")).toBeInTheDocument();
    expect(screen.getByText("docs")).toBeInTheDocument();
    expect(screen.getByText("file.txt")).toBeInTheDocument();
  });

  it("calls onNavigate when clicking a directory", () => {
    const onNavigate = vi.fn();
    render(<ColumnView entries={entries} onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText("docs"));
    expect(onNavigate).toHaveBeenCalledWith(entries[0]);
  });

  it("shows chevron indicator for directories", () => {
    render(<ColumnView entries={entries} onNavigate={vi.fn()} />);
    const dirRow = screen.getByText("docs").closest("[role='listitem']");
    expect(dirRow?.textContent).toContain(">");
  });
});
