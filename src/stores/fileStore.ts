import { create } from "zustand";
import type { FileEntry } from "../types/file";

const MAX_RECENTS = 20;

interface FileState {
  currentPath: string;
  entries: FileEntry[];
  recentPaths: string[];
  selectedFiles: string[];
  error: string | null;
  loading: boolean;
  navigateTo: (path: string) => void;
  goUp: () => void;
  setEntries: (entries: FileEntry[]) => void;
  setSelectedFiles: (paths: string[]) => void;
  setError: (error: string) => void;
  clearError: () => void;
  setLoading: (loading: boolean) => void;
}

export const useFileStore = create<FileState>()((set, get) => ({
  currentPath: "",
  entries: [],
  recentPaths: [],
  selectedFiles: [],
  error: null,
  loading: false,

  navigateTo: (path) =>
    set((s) => ({
      currentPath: path,
      error: null,
      recentPaths: [path, ...s.recentPaths.filter((p) => p !== path)].slice(
        0,
        MAX_RECENTS,
      ),
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
}));
