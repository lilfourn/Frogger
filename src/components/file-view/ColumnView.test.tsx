import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ColumnView } from "./ColumnView";
import type { FileEntry } from "../../types/file";

vi.mock("../../services/fileService", () => ({
  listDirectory: vi.fn(),
}));

vi.mock("../../stores/fileStore", () => ({
  useFileStore: vi.fn((selector) => {
    const state = {
      currentPath: "/root",
      navigateTo: vi.fn(),
    };
    return selector(state);
  }),
}));

import { listDirectory } from "../../services/fileService";
import { useFileStore } from "../../stores/fileStore";

function makeEntry(name: string, parentPath: string, isDir = false): FileEntry {
  return {
    path: `${parentPath}/${name}`,
    name,
    extension: isDir ? null : (name.split(".").pop() ?? null),
    mime_type: null,
    size_bytes: 100,
    created_at: null,
    modified_at: null,
    is_directory: isDir,
    parent_path: parentPath,
  };
}

const rootEntries = [
  makeEntry("docs", "/root", true),
  makeEntry("file.txt", "/root"),
];

const docsEntries = [
  makeEntry("readme.md", "/root/docs"),
  makeEntry("sub", "/root/docs", true),
];

function defaultProps(overrides: Partial<Parameters<typeof ColumnView>[0]> = {}) {
  return {
    entries: rootEntries,
    onSelect: vi.fn(),
    onOpen: vi.fn(),
    onItemContextMenu: vi.fn(),
    selectedPaths: new Set<string>(),
    ...overrides,
  };
}

describe("ColumnView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listDirectory).mockResolvedValue(docsEntries);
    vi.mocked(useFileStore).mockImplementation((selector) => {
      const state = {
        currentPath: "/root",
        navigateTo: vi.fn(),
      };
      return selector(state as never);
    });
  });

  it("renders initial column with entries", () => {
    render(<ColumnView {...defaultProps()} />);
    expect(screen.getByTestId("column-view")).toBeInTheDocument();
    expect(screen.getByText("docs")).toBeInTheDocument();
    expect(screen.getByText("file.txt")).toBeInTheDocument();
  });

  it("shows chevron indicator for directories", () => {
    render(<ColumnView {...defaultProps()} />);
    const dirRow = screen.getByText("docs").closest("[role='listitem']");
    expect(dirRow?.textContent).toContain(">");
    const fileRow = screen.getByText("file.txt").closest("[role='listitem']");
    expect(fileRow?.textContent).not.toContain(">");
  });

  it("clicking directory loads child column", async () => {
    render(<ColumnView {...defaultProps()} />);
    fireEvent.click(screen.getByText("docs"));

    await waitFor(() => {
      expect(listDirectory).toHaveBeenCalledWith("/root/docs");
      expect(screen.getByText("readme.md")).toBeInTheDocument();
    });
  });

  it("clicking directory truncates columns to the right", async () => {
    vi.mocked(listDirectory)
      .mockResolvedValueOnce(docsEntries)
      .mockResolvedValueOnce([makeEntry("deep.txt", "/root/docs/sub")]);

    render(<ColumnView {...defaultProps()} />);

    fireEvent.click(screen.getByText("docs"));
    await waitFor(() => expect(screen.getByText("readme.md")).toBeInTheDocument());

    fireEvent.click(screen.getByText("sub"));
    await waitFor(() => expect(screen.getByText("deep.txt")).toBeInTheDocument());

    // Click docs again — should truncate sub's children
    vi.mocked(listDirectory).mockResolvedValueOnce(docsEntries);
    fireEvent.click(screen.getByText("docs"));
    await waitFor(() => {
      expect(screen.queryByText("deep.txt")).not.toBeInTheDocument();
    });
  });

  it("clicking file selects without adding column", async () => {
    const onSelect = vi.fn();
    render(<ColumnView {...defaultProps({ onSelect })} />);
    fireEvent.click(screen.getByText("file.txt"));
    expect(onSelect).toHaveBeenCalledWith(rootEntries[1]);
    // Should not trigger listDirectory for a file
    expect(listDirectory).not.toHaveBeenCalled();
  });

  it("double-click calls onOpen", () => {
    const onOpen = vi.fn();
    render(<ColumnView {...defaultProps({ onOpen })} />);
    fireEvent.doubleClick(screen.getByText("file.txt"));
    expect(onOpen).toHaveBeenCalledWith(rootEntries[1]);
  });

  it("right-click calls onItemContextMenu", () => {
    const onItemContextMenu = vi.fn();
    render(<ColumnView {...defaultProps({ onItemContextMenu })} />);
    fireEvent.contextMenu(screen.getByText("file.txt"));
    expect(onItemContextMenu).toHaveBeenCalled();
    expect(onItemContextMenu.mock.calls[0][1]).toEqual(rootEntries[1]);
  });

  it("preserves columns through two-phase entries update (empty → real)", async () => {
    let storePath = "/root";
    const navigateTo = vi.fn((p: string) => { storePath = p; });

    vi.mocked(useFileStore).mockImplementation((selector) => {
      const state = { currentPath: storePath, navigateTo };
      return selector(state as never);
    });

    const { rerender } = render(<ColumnView {...defaultProps()} />);

    // Click "docs" → loads child column
    fireEvent.click(screen.getByText("docs"));
    await waitFor(() => expect(screen.getByText("readme.md")).toBeInTheDocument());

    // navigateTo was called → store path changes
    expect(navigateTo).toHaveBeenCalledWith("/root/docs");

    // Phase 1: store fires entries=[] while loading
    rerender(<ColumnView {...defaultProps({ entries: [] })} />);

    // Phase 2: real entries arrive
    rerender(<ColumnView {...defaultProps({ entries: docsEntries })} />);

    // Child column (readme.md) should still be visible
    expect(screen.getByText("readme.md")).toBeInTheDocument();
  });

  it("shows selected styling for selected items", () => {
    const selectedPaths = new Set(["/root/docs"]);
    render(<ColumnView {...defaultProps({ selectedPaths })} />);
    const dirRow = screen.getByText("docs").closest("[role='listitem']");
    expect(dirRow?.className).toContain("bg-[var(--color-accent)]");
  });
});
