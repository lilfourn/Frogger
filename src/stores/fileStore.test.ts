import { describe, it, expect, beforeEach } from "vitest";
import { useFileStore } from "./fileStore";

describe("fileStore", () => {
  beforeEach(() => {
    useFileStore.setState(useFileStore.getInitialState());
  });

  it("has correct defaults", () => {
    const state = useFileStore.getState();
    expect(state.currentPath).toBe("");
    expect(state.entries).toEqual([]);
    expect(state.recentPaths).toEqual([]);
    expect(state.selectedFiles).toEqual([]);
    expect(state.error).toBeNull();
    expect(state.loading).toBe(false);
  });

  it("navigateTo updates currentPath and pushes to recents", () => {
    useFileStore.getState().navigateTo("/Users");
    expect(useFileStore.getState().currentPath).toBe("/Users");
    expect(useFileStore.getState().recentPaths).toEqual(["/Users"]);

    useFileStore.getState().navigateTo("/tmp");
    expect(useFileStore.getState().currentPath).toBe("/tmp");
    expect(useFileStore.getState().recentPaths).toEqual(["/tmp", "/Users"]);
  });

  it("navigateTo deduplicates recents (most recent first)", () => {
    useFileStore.getState().navigateTo("/Users");
    useFileStore.getState().navigateTo("/tmp");
    useFileStore.getState().navigateTo("/Users");
    expect(useFileStore.getState().recentPaths).toEqual(["/Users", "/tmp"]);
  });

  it("recents capped at 20", () => {
    for (let i = 0; i < 25; i++) {
      useFileStore.getState().navigateTo(`/path/${i}`);
    }
    expect(useFileStore.getState().recentPaths).toHaveLength(20);
    expect(useFileStore.getState().recentPaths[0]).toBe("/path/24");
  });

  it("setEntries updates entries", () => {
    const entries = [
      {
        path: "/test/file.txt",
        name: "file.txt",
        extension: "txt",
        mime_type: "text/plain",
        size_bytes: 100,
        created_at: null,
        modified_at: null,
        is_directory: false,
        parent_path: "/test",
      },
    ];
    useFileStore.getState().setEntries(entries);
    expect(useFileStore.getState().entries).toEqual(entries);
  });

  it("setSelectedFiles updates selection", () => {
    useFileStore.getState().setSelectedFiles(["/a", "/b"]);
    expect(useFileStore.getState().selectedFiles).toEqual(["/a", "/b"]);
  });

  it("setError and clearError work", () => {
    useFileStore.getState().setError("something broke");
    expect(useFileStore.getState().error).toBe("something broke");

    useFileStore.getState().clearError();
    expect(useFileStore.getState().error).toBeNull();
  });

  it("setLoading updates loading state", () => {
    useFileStore.getState().setLoading(true);
    expect(useFileStore.getState().loading).toBe(true);
  });

  it("goUp navigates to parent directory", () => {
    useFileStore.getState().navigateTo("/Users/test/deep");
    useFileStore.getState().goUp();
    expect(useFileStore.getState().currentPath).toBe("/Users/test");
  });

  it("goUp at root stays at root", () => {
    useFileStore.getState().navigateTo("/");
    useFileStore.getState().goUp();
    expect(useFileStore.getState().currentPath).toBe("/");
  });

  it("sortBy and sortDirection have correct defaults", () => {
    expect(useFileStore.getState().sortBy).toBe("name");
    expect(useFileStore.getState().sortDirection).toBe("asc");
  });

  it("setSortBy updates sort field", () => {
    useFileStore.getState().setSortBy("size");
    expect(useFileStore.getState().sortBy).toBe("size");
  });

  it("toggleSortDirection flips asc/desc", () => {
    useFileStore.getState().toggleSortDirection();
    expect(useFileStore.getState().sortDirection).toBe("desc");
    useFileStore.getState().toggleSortDirection();
    expect(useFileStore.getState().sortDirection).toBe("asc");
  });

  it("sortedEntries sorts by name (dirs first)", () => {
    useFileStore.getState().setEntries([
      {
        path: "/b.txt",
        name: "b.txt",
        extension: "txt",
        mime_type: null,
        size_bytes: 200,
        created_at: null,
        modified_at: null,
        is_directory: false,
        parent_path: "/",
      },
      {
        path: "/a.txt",
        name: "a.txt",
        extension: "txt",
        mime_type: null,
        size_bytes: 100,
        created_at: null,
        modified_at: null,
        is_directory: false,
        parent_path: "/",
      },
      {
        path: "/zdir",
        name: "zdir",
        extension: null,
        mime_type: null,
        size_bytes: null,
        created_at: null,
        modified_at: null,
        is_directory: true,
        parent_path: "/",
      },
    ]);
    const sorted = useFileStore.getState().sortedEntries();
    expect(sorted[0].name).toBe("zdir");
    expect(sorted[1].name).toBe("a.txt");
    expect(sorted[2].name).toBe("b.txt");
  });

  it("sortedEntries sorts by size", () => {
    useFileStore.getState().setEntries([
      {
        path: "/big.txt",
        name: "big.txt",
        extension: "txt",
        mime_type: null,
        size_bytes: 9999,
        created_at: null,
        modified_at: null,
        is_directory: false,
        parent_path: "/",
      },
      {
        path: "/small.txt",
        name: "small.txt",
        extension: "txt",
        mime_type: null,
        size_bytes: 10,
        created_at: null,
        modified_at: null,
        is_directory: false,
        parent_path: "/",
      },
    ]);
    useFileStore.getState().setSortBy("size");
    const sorted = useFileStore.getState().sortedEntries();
    expect(sorted[0].name).toBe("small.txt");
    expect(sorted[1].name).toBe("big.txt");
  });

  // --- Tab tests ---

  it("starts with one default tab", () => {
    useFileStore.getState().navigateTo("/Users");
    const { tabs, activeTabId } = useFileStore.getState();
    expect(tabs).toHaveLength(1);
    expect(tabs[0].id).toBe(activeTabId);
    expect(tabs[0].path).toBe("/Users");
  });

  it("addTab creates a new tab at current path", () => {
    useFileStore.getState().navigateTo("/Users");
    useFileStore.getState().addTab();
    const { tabs, activeTabId } = useFileStore.getState();
    expect(tabs).toHaveLength(2);
    expect(tabs[1].id).toBe(activeTabId);
    expect(tabs[1].path).toBe("/Users");
  });

  it("switchTab changes active tab and currentPath", () => {
    useFileStore.getState().navigateTo("/Users");
    useFileStore.getState().addTab();
    useFileStore.getState().navigateTo("/tmp");

    const firstTabId = useFileStore.getState().tabs[0].id;
    useFileStore.getState().switchTab(firstTabId);

    expect(useFileStore.getState().activeTabId).toBe(firstTabId);
    expect(useFileStore.getState().currentPath).toBe("/Users");
  });

  it("closeTab removes tab and switches to adjacent", () => {
    useFileStore.getState().navigateTo("/Users");
    useFileStore.getState().addTab();
    useFileStore.getState().navigateTo("/tmp");

    const secondTabId = useFileStore.getState().activeTabId;
    useFileStore.getState().closeTab(secondTabId);

    expect(useFileStore.getState().tabs).toHaveLength(1);
    expect(useFileStore.getState().currentPath).toBe("/Users");
  });

  it("closeTab does not remove last tab", () => {
    useFileStore.getState().navigateTo("/Users");
    const tabId = useFileStore.getState().activeTabId;
    useFileStore.getState().closeTab(tabId);
    expect(useFileStore.getState().tabs).toHaveLength(1);
  });

  it("navigateTo clears entries and selectedFiles", () => {
    useFileStore.getState().setEntries([
      {
        path: "/old/file.txt",
        name: "file.txt",
        extension: "txt",
        mime_type: null,
        size_bytes: 100,
        created_at: null,
        modified_at: null,
        is_directory: false,
        parent_path: "/old",
      },
    ]);
    useFileStore.getState().setSelectedFiles(["/old/file.txt"]);
    expect(useFileStore.getState().entries).toHaveLength(1);
    expect(useFileStore.getState().selectedFiles).toHaveLength(1);

    useFileStore.getState().navigateTo("/new/path");
    expect(useFileStore.getState().entries).toEqual([]);
    expect(useFileStore.getState().selectedFiles).toEqual([]);
  });

  it("navigateTo updates the active tab path", () => {
    useFileStore.getState().navigateTo("/Users");
    useFileStore.getState().navigateTo("/tmp");
    const activeTab = useFileStore
      .getState()
      .tabs.find((t) => t.id === useFileStore.getState().activeTabId);
    expect(activeTab?.path).toBe("/tmp");
  });
});
