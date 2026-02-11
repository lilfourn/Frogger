import { create } from "zustand";

export type Theme = "light" | "dark" | "system";
export type ViewMode = "list" | "grid" | "column" | "gallery";

interface SettingsState {
  theme: Theme;
  viewMode: ViewMode;
  sidebarWidth: number;
  sidebarVisible: boolean;
  showHiddenFiles: boolean;
  setTheme: (theme: Theme) => void;
  setViewMode: (mode: ViewMode) => void;
  setSidebarWidth: (width: number) => void;
  toggleSidebar: () => void;
  toggleHiddenFiles: () => void;
  resolvedTheme: () => "light" | "dark";
}

function getSystemTheme(): "light" | "dark" {
  if (typeof window === "undefined") return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export const useSettingsStore = create<SettingsState>()((set, get) => ({
  theme: "system",
  viewMode: "list",
  sidebarWidth: 240,
  sidebarVisible: true,
  showHiddenFiles: false,
  setTheme: (theme) => set({ theme }),
  setViewMode: (mode) => set({ viewMode: mode }),
  setSidebarWidth: (width) => set({ sidebarWidth: width }),
  toggleSidebar: () => set((s) => ({ sidebarVisible: !s.sidebarVisible })),
  toggleHiddenFiles: () => set((s) => ({ showHiddenFiles: !s.showHiddenFiles })),
  resolvedTheme: () => {
    const { theme } = get();
    return theme === "system" ? getSystemTheme() : theme;
  },
}));
