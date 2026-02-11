import { create } from "zustand";
import type { FileEntry } from "../types/file";

const MAX_RECENTS = 20;

export type SortField = "name" | "size" | "date" | "kind";
export type SortDirection = "asc" | "desc";

interface FileState {
  currentPath: string;
  entries: FileEntry[];
  recentPaths: string[];
  selectedFiles: string[];
  error: string | null;
  loading: boolean;
  sortBy: SortField;
  sortDirection: SortDirection;
  navigateTo: (path: string) => void;
  goUp: () => void;
  setEntries: (entries: FileEntry[]) => void;
  setSelectedFiles: (paths: string[]) => void;
  setError: (error: string) => void;
  clearError: () => void;
  setLoading: (loading: boolean) => void;
  setSortBy: (field: SortField) => void;
  toggleSortDirection: () => void;
  sortedEntries: () => FileEntry[];
}

export const useFileStore = create<FileState>()((set, get) => ({
  currentPath: "",
  entries: [],
  recentPaths: [],
  selectedFiles: [],
  error: null,
  loading: false,
  sortBy: "name",
  sortDirection: "asc",

  navigateTo: (path) =>
    set((s) => ({
      currentPath: path,
      error: null,
      recentPaths: [path, ...s.recentPaths.filter((p) => p !== path)].slice(0, MAX_RECENTS),
    })),

  goUp: () => {
    const { currentPath, navigateTo } = get();
    const parent = currentPath.replace(/\/[^/]+\/?$/, "") || "/";
    if (parent !== currentPath) navigateTo(parent);
  },

  setEntries: (entries) => set({ entries }),
  setSelectedFiles: (paths) => set({ selectedFiles: paths }),
  setError: (error) => set({ error }),
  clearError: () => set({ error: null }),
  setLoading: (loading) => set({ loading }),
  setSortBy: (field) => set({ sortBy: field }),
  toggleSortDirection: () =>
    set((s) => ({ sortDirection: s.sortDirection === "asc" ? "desc" : "asc" })),

  sortedEntries: () => {
    const { entries, sortBy, sortDirection } = get();
    const dirs = entries.filter((e) => e.is_directory);
    const files = entries.filter((e) => !e.is_directory);
    const mul = sortDirection === "asc" ? 1 : -1;

    const compare = (a: FileEntry, b: FileEntry): number => {
      switch (sortBy) {
        case "size":
          return ((a.size_bytes ?? 0) - (b.size_bytes ?? 0)) * mul;
        case "date":
          return (a.modified_at ?? "").localeCompare(b.modified_at ?? "") * mul;
        case "kind":
          return (a.extension ?? "").localeCompare(b.extension ?? "") * mul;
        default:
          return a.name.toLowerCase().localeCompare(b.name.toLowerCase()) * mul;
      }
    };

    return [...dirs.sort(compare), ...files.sort(compare)];
  },
}));
