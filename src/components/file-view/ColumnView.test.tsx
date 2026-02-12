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

const defaultProps = {
  onSelect: vi.fn(),
  onOpen: vi.fn(),
  onItemContextMenu: vi.fn(),
  selectedPaths: new Set<string>(),
};

describe("ColumnView", () => {
  const entries = [makeEntry("docs", true), makeEntry("file.txt")];

  it("renders entries in a column", () => {
    render(<ColumnView entries={entries} {...defaultProps} />);
    expect(screen.getByTestId("column-view")).toBeInTheDocument();
    expect(screen.getByText("docs")).toBeInTheDocument();
    expect(screen.getByText("file.txt")).toBeInTheDocument();
  });

  it("calls onSelect on single click", () => {
    const onSelect = vi.fn();
    render(<ColumnView entries={entries} {...defaultProps} onSelect={onSelect} />);
    fireEvent.click(screen.getByText("docs"));
    expect(onSelect).toHaveBeenCalledWith(entries[0]);
  });

  it("calls onOpen on double click", () => {
    const onOpen = vi.fn();
    render(<ColumnView entries={entries} {...defaultProps} onOpen={onOpen} />);
    fireEvent.doubleClick(screen.getByText("docs"));
    expect(onOpen).toHaveBeenCalledWith(entries[0]);
  });

  it("calls onItemContextMenu on right click", () => {
    const onItemContextMenu = vi.fn();
    render(
      <ColumnView entries={entries} {...defaultProps} onItemContextMenu={onItemContextMenu} />,
    );
    fireEvent.contextMenu(screen.getByText("file.txt"));
    expect(onItemContextMenu).toHaveBeenCalled();
    expect(onItemContextMenu.mock.calls[0][1]).toEqual(entries[1]);
  });

  it("shows selected styling for selected items", () => {
    const selectedPaths = new Set(["/docs"]);
    render(<ColumnView entries={entries} {...defaultProps} selectedPaths={selectedPaths} />);
    const dirRow = screen.getByText("docs").closest("[role='listitem']");
    expect(dirRow?.className).toContain("bg-[var(--color-accent)]/10");
  });

  it("shows chevron indicator for directories", () => {
    render(<ColumnView entries={entries} {...defaultProps} />);
    const dirRow = screen.getByText("docs").closest("[role='listitem']");
    expect(dirRow?.textContent).toContain(">");
  });
});
