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
});
