import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useFileStore } from "../stores/fileStore";
import { useSearchStore } from "../stores/searchStore";

vi.mock("../services/searchService", () => ({
  search: vi.fn(),
}));

import { search } from "../services/searchService";
import { useSearch } from "./useSearch";

const mockSearch = vi.mocked(search);

describe("useSearch", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useSearchStore.setState(useSearchStore.getInitialState());
    useFileStore.setState({ currentPath: "" });
    mockSearch.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not search for queries shorter than 2 chars", () => {
    useSearchStore.getState().setQuery("a");
    renderHook(() => useSearch());

    vi.advanceTimersByTime(200);

    expect(mockSearch).not.toHaveBeenCalled();
    expect(useSearchStore.getState().results).toEqual([]);
  });

  it("searches after 150ms debounce", async () => {
    const results = [
      {
        file_path: "/test.txt",
        file_name: "test.txt",
        is_directory: false,
        score: 1,
        match_source: "fts" as const,
        snippet: null,
      },
    ];
    mockSearch.mockResolvedValueOnce(results);

    useSearchStore.getState().setQuery("te");
    renderHook(() => useSearch());

    expect(mockSearch).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(150);
    });

    expect(mockSearch).toHaveBeenCalledWith("te", 20, undefined);
  });

  it("clears results for empty query", () => {
    useSearchStore.getState().setResults([
      {
        file_path: "/x",
        file_name: "x",
        is_directory: false,
        score: 1,
        match_source: "fts",
        snippet: null,
      },
    ]);
    useSearchStore.getState().setQuery("");

    renderHook(() => useSearch());

    expect(useSearchStore.getState().results).toEqual([]);
  });

  it("always searches with latest query value", async () => {
    mockSearch.mockResolvedValue([]);

    renderHook(() => useSearch());

    await act(async () => {
      useSearchStore.getState().setQuery("first");
    });

    await act(async () => {
      useSearchStore.getState().setQuery("second");
    });

    await act(async () => {
      vi.advanceTimersByTime(200);
    });

    expect(mockSearch).toHaveBeenLastCalledWith("second", 20, undefined);
  });

  it("handles search errors gracefully", async () => {
    mockSearch.mockRejectedValueOnce(new Error("network error"));
    useSearchStore.getState().setQuery("fail");

    renderHook(() => useSearch());

    await act(async () => {
      vi.advanceTimersByTime(150);
    });

    await vi.waitFor(() => {
      expect(useSearchStore.getState().isSearching).toBe(false);
    });

    expect(useSearchStore.getState().results).toEqual([]);
    expect(useSearchStore.getState().isSearching).toBe(false);
  });
});
