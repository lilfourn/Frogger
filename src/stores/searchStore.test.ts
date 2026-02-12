import { describe, it, expect, beforeEach } from "vitest";
import { useSearchStore } from "./searchStore";

describe("searchStore", () => {
  beforeEach(() => {
    useSearchStore.setState(useSearchStore.getInitialState());
  });

  it("has correct defaults", () => {
    const state = useSearchStore.getState();
    expect(state.query).toBe("");
    expect(state.results).toEqual([]);
    expect(state.isSearching).toBe(false);
    expect(state.isOpen).toBe(false);
    expect(state.selectedIndex).toBe(0);
  });

  it("setQuery updates query and resets selectedIndex", () => {
    useSearchStore.getState().setSelectedIndex(3);
    useSearchStore.getState().setQuery("test");
    expect(useSearchStore.getState().query).toBe("test");
    expect(useSearchStore.getState().selectedIndex).toBe(0);
  });

  it("setResults updates results", () => {
    const results = [
      {
        file_path: "/a.txt",
        file_name: "a.txt",
        is_directory: false,
        score: 1,
        match_source: "fts" as const,
        snippet: null,
      },
    ];
    useSearchStore.getState().setResults(results);
    expect(useSearchStore.getState().results).toEqual(results);
  });

  it("setIsSearching updates searching state", () => {
    useSearchStore.getState().setIsSearching(true);
    expect(useSearchStore.getState().isSearching).toBe(true);
  });

  it("setSelectedIndex updates index", () => {
    useSearchStore.getState().setSelectedIndex(5);
    expect(useSearchStore.getState().selectedIndex).toBe(5);
  });

  it("open resets state and sets isOpen", () => {
    useSearchStore.getState().setQuery("old");
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
    useSearchStore.getState().setSelectedIndex(2);

    useSearchStore.getState().open();

    const state = useSearchStore.getState();
    expect(state.isOpen).toBe(true);
    expect(state.query).toBe("");
    expect(state.results).toEqual([]);
    expect(state.selectedIndex).toBe(0);
  });

  it("close resets state and clears isOpen", () => {
    useSearchStore.getState().open();
    useSearchStore.getState().setQuery("search");

    useSearchStore.getState().close();

    const state = useSearchStore.getState();
    expect(state.isOpen).toBe(false);
    expect(state.query).toBe("");
    expect(state.results).toEqual([]);
    expect(state.selectedIndex).toBe(0);
  });

  it("clear resets query/results/searching but not isOpen", () => {
    useSearchStore.getState().open();
    useSearchStore.getState().setQuery("test");
    useSearchStore.getState().setIsSearching(true);

    useSearchStore.getState().clear();

    const state = useSearchStore.getState();
    expect(state.isOpen).toBe(true);
    expect(state.query).toBe("");
    expect(state.results).toEqual([]);
    expect(state.isSearching).toBe(false);
    expect(state.selectedIndex).toBe(0);
  });
});
