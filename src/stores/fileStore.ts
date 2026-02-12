import { create } from "zustand";
import type { FileEntry } from "../types/file";
import { useSettingsStore } from "./settingsStore";

const MAX_RECENTS = 20;

let tabIdCounter = 1;
function nextTabId(): string {
  return `tab-${tabIdCounter++}`;
}

export type SortField = "name" | "size" | "date" | "kind";
export type SortDirection = "asc" | "desc";

export interface Tab {
  id: string;
  path: string;
}

interface FileState {
  currentPath: string;
  entries: FileEntry[];
  recentPaths: string[];
  selectedFiles: string[];
  error: string | null;
  loading: boolean;
  sortBy: SortField;
  sortDirection: SortDirection;
  tabs: Tab[];
  activeTabId: string;
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
  addTab: () => void;
  closeTab: (id: string) => void;
  switchTab: (id: string) => void;
}

const defaultTabId = nextTabId();

export const useFileStore = create<FileState>()((set, get) => ({
  currentPath: "",
  entries: [],
  recentPaths: [],
  selectedFiles: [],
  error: null,
  loading: false,
  sortBy: "name",
  sortDirection: "asc",
  tabs: [{ id: defaultTabId, path: "" }],
  activeTabId: defaultTabId,

  navigateTo: (path) =>
    set((s) => ({
      currentPath: path,
      entries: [],
      selectedFiles: [],
      error: null,
      recentPaths: [path, ...s.recentPaths.filter((p) => p !== path)].slice(0, MAX_RECENTS),
      tabs: s.tabs.map((t) => (t.id === s.activeTabId ? { ...t, path } : t)),
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
    const { showHiddenFiles } = useSettingsStore.getState();
    const visible = showHiddenFiles ? entries : entries.filter((e) => !e.name.startsWith("."));
    const dirs = visible.filter((e) => e.is_directory);
    const files = visible.filter((e) => !e.is_directory);
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

  addTab: () => {
    const { currentPath } = get();
    const id = nextTabId();
    set((s) => ({
      tabs: [...s.tabs, { id, path: currentPath }],
      activeTabId: id,
    }));
  },

  closeTab: (id) => {
    const { tabs, activeTabId } = get();
    if (tabs.length <= 1) return;
    const idx = tabs.findIndex((t) => t.id === id);
    const remaining = tabs.filter((t) => t.id !== id);
    if (id === activeTabId) {
      const newActive = remaining[Math.min(idx, remaining.length - 1)];
      set({
        tabs: remaining,
        activeTabId: newActive.id,
        currentPath: newActive.path,
      });
    } else {
      set({ tabs: remaining });
    }
  },

  switchTab: (id) => {
    const tab = get().tabs.find((t) => t.id === id);
    if (tab) {
      set({ activeTabId: id, currentPath: tab.path });
    }
  },
}));
