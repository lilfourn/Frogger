import { create } from "zustand";
import type { SearchResult } from "../types/search";

interface SearchState {
  query: string;
  results: SearchResult[];
  isSearching: boolean;
  isOpen: boolean;
  selectedIndex: number;
  setQuery: (query: string) => void;
  setResults: (results: SearchResult[]) => void;
  setIsSearching: (searching: boolean) => void;
  setSelectedIndex: (index: number) => void;
  open: () => void;
  close: () => void;
  clear: () => void;
}

export const useSearchStore = create<SearchState>()((set) => ({
  query: "",
  results: [],
  isSearching: false,
  isOpen: false,
  selectedIndex: 0,
  setQuery: (query) => set({ query, selectedIndex: 0 }),
  setResults: (results) => set({ results }),
  setIsSearching: (isSearching) => set({ isSearching }),
  setSelectedIndex: (selectedIndex) => set({ selectedIndex }),
  open: () => set({ isOpen: true, query: "", results: [], selectedIndex: 0 }),
  close: () => set({ isOpen: false, query: "", results: [], selectedIndex: 0 }),
  clear: () => set({ query: "", results: [], isSearching: false, selectedIndex: 0 }),
}));
