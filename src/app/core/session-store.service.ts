import { Injectable, computed, inject, signal } from "@angular/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { FroggerApiService } from "./frogger-api.service";
import type {
  AppBootstrap,
  AppSettings,
  FolderViewState,
  SidebarState,
  SortState,
  TabState,
  ViewMode,
  WindowState,
} from "./frogger-api.types";

@Injectable({ providedIn: "root" })
export class SessionStoreService {
  private readonly api = inject(FroggerApiService);

  readonly bootstrap = signal<AppBootstrap | null>(null);
  readonly windows = signal<WindowState[]>([]);
  readonly activeWindowId = signal<string | null>(null);
  readonly persistenceError = signal<string | null>(null);

  readonly activeWindow = computed(() => {
    const activeId = this.activeWindowId();
    return this.windows().find((window) => window.id === activeId) ?? this.windows()[0] ?? null;
  });

  readonly activeTab = computed(() => {
    const window = this.activeWindow();
    if (!window) {
      return null;
    }

    return window.tabs.find((tab) => tab.id === window.activeTabId) ?? window.tabs[0] ?? null;
  });

  initialize(bootstrap: AppBootstrap): void {
    const currentLabel = this.currentWindowLabel();
    const currentWindow = bootstrap.windows.find((window) => window.label === currentLabel);

    this.bootstrap.set(bootstrap);
    this.windows.set(bootstrap.windows);
    this.activeWindowId.set(currentWindow?.id ?? bootstrap.windows[0]?.id ?? null);
    this.persistenceError.set(null);
  }

  switchWindow(windowId: string): void {
    if (this.windows().some((window) => window.id === windowId)) {
      this.activeWindowId.set(windowId);
    }
  }

  switchTab(windowId: string, tabId: string): void {
    this.windows.update((windows) =>
      windows.map((window) => {
        if (window.id !== windowId || !window.tabs.some((tab) => tab.id === tabId)) {
          return window;
        }

        return {
          ...window,
          activeTabId: tabId,
          tabs: window.tabs.map((tab) => ({ ...tab, isActive: tab.id === tabId })),
        };
      }),
    );
    void this.persist();
  }

  openTab(windowId: string, path: string, title = this.titleFromPath(path)): void {
    this.windows.update((windows) =>
      windows.map((window) => {
        if (window.id !== windowId) {
          return window;
        }

        const tab: TabState = {
          id: this.id("tab"),
          path,
          title,
          position: window.tabs.length,
          isActive: true,
          folderState: this.defaultFolderState(),
        };

        return {
          ...window,
          activeTabId: tab.id,
          tabs: [...window.tabs.map((existing) => ({ ...existing, isActive: false })), tab],
        };
      }),
    );
    void this.persist();
  }

  closeTab(windowId: string, tabId: string): void {
    this.windows.update((windows) =>
      windows.map((window) => {
        if (window.id !== windowId || window.tabs.length <= 1) {
          return window;
        }

        const closingIndex = window.tabs.findIndex((tab) => tab.id === tabId);
        if (closingIndex < 0) {
          return window;
        }

        const remainingTabs = window.tabs.filter((tab) => tab.id !== tabId);
        const nextActiveTab =
          window.activeTabId === tabId
            ? remainingTabs[Math.max(0, closingIndex - 1)]
            : remainingTabs.find((tab) => tab.id === window.activeTabId) ?? remainingTabs[0];

        return {
          ...window,
          activeTabId: nextActiveTab.id,
          tabs: remainingTabs.map((tab, index) => ({
            ...tab,
            position: index,
            isActive: tab.id === nextActiveTab.id,
          })),
        };
      }),
    );
    void this.persist();
  }

  async createWindow(path: string | null = null): Promise<void> {
    await this.persist();
    const window = await this.api.createFileManagerWindow(path);
    this.windows.update((windows) => {
      const existingIndex = windows.findIndex(
        (candidate) => candidate.id === window.id || candidate.label === window.label,
      );
      if (existingIndex < 0) {
        return [...windows, window];
      }

      return windows.map((candidate, index) => (index === existingIndex ? window : candidate));
    });
  }

  updateSettings(settings: AppSettings): void {
    this.bootstrap.update((bootstrap) => (bootstrap ? { ...bootstrap, settings } : bootstrap));
  }

  updateActiveTabPath(
    path: string,
    title = this.titleFromPath(path),
    folderState: FolderViewState | null = null,
  ): void {
    const activeWindow = this.activeWindow();
    const activeTab = this.activeTab();
    if (!activeWindow || !activeTab) {
      return;
    }

    this.windows.update((windows) =>
      windows.map((window) => {
        if (window.id !== activeWindow.id) {
          return window;
        }

        return {
          ...window,
          tabs: window.tabs.map((tab) =>
            tab.id === activeTab.id
              ? {
                  ...tab,
                  path,
                  title,
                  folderState: folderState ?? {
                    ...tab.folderState,
                    scrollOffset: 0,
                    selectedItemPath: null,
                  },
                }
              : tab,
          ),
        };
      }),
    );
    void this.persist();
  }

  updateSidebarState(sidebar: SidebarState): void {
    this.bootstrap.update((bootstrap) => (bootstrap ? { ...bootstrap, sidebar } : bootstrap));
  }

  updateSidebarCollapsed(collapsed: boolean): void {
    const activeWindow = this.activeWindow();
    if (!activeWindow) {
      return;
    }

    this.windows.update((windows) =>
      windows.map((window) =>
        window.id === activeWindow.id ? { ...window, sidebarCollapsed: collapsed } : window,
      ),
    );
    void this.persist();
  }

  updateSidebarWidth(width: number): void {
    const activeWindow = this.activeWindow();
    if (!activeWindow) {
      return;
    }

    this.windows.update((windows) =>
      windows.map((window) =>
        window.id === activeWindow.id ? { ...window, sidebarWidth: width } : window,
      ),
    );
    void this.persist();
  }

  updateActiveTabFolderState(folderState: FolderViewState): void {
    const activeWindow = this.activeWindow();
    const activeTab = this.activeTab();
    if (!activeWindow || !activeTab) {
      return;
    }

    this.windows.update((windows) =>
      windows.map((window) => {
        if (window.id !== activeWindow.id) {
          return window;
        }

        return {
          ...window,
          tabs: window.tabs.map((tab) =>
            tab.id === activeTab.id ? { ...tab, folderState } : tab,
          ),
        };
      }),
    );
    void this.persist();
    void this.api.saveFolderViewState(activeTab.path, folderState);
  }

  updateActiveTabViewMode(viewMode: ViewMode): void {
    const activeWindow = this.activeWindow();
    const activeTab = this.activeTab();
    if (!activeWindow || !activeTab) {
      return;
    }

    this.windows.update((windows) =>
      windows.map((window) => {
        if (window.id !== activeWindow.id) {
          return window;
        }

        return {
          ...window,
          tabs: window.tabs.map((tab) =>
            tab.id === activeTab.id
              ? { ...tab, folderState: { ...tab.folderState, viewMode } }
              : tab,
          ),
        };
      }),
    );
    void this.persist();
  }

  private async persist(): Promise<void> {
    try {
      await this.api.saveSessionState(this.windows());
      this.persistenceError.set(null);
    } catch (error: unknown) {
      this.persistenceError.set(this.toErrorMessage(error));
    }
  }

  private defaultFolderState(): FolderViewState {
    const settings = this.bootstrap()?.settings;
    const sort: SortState = { key: "name", direction: "asc" };

    return {
      viewMode: "list",
      sort,
      foldersFirst: settings?.foldersFirst ?? true,
      hiddenFilesVisible: settings?.hiddenFilesVisible ?? false,
      fileExtensionsVisible: settings?.fileExtensionsVisible ?? false,
      scrollOffset: 0,
      selectedItemPath: null,
    };
  }

  private titleFromPath(path: string): string {
    const normalized = path.replace(/[/\\]+$/, "");
    const parts = normalized.split(/[/\\]/);
    return parts.at(-1) || "Home";
  }

  private id(prefix: string): string {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      return `${prefix}-${crypto.randomUUID()}`;
    }

    return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  }

  private currentWindowLabel(): string | null {
    try {
      return getCurrentWindow().label;
    } catch {
      return null;
    }
  }

  private toErrorMessage(error: unknown): string {
    if (typeof error === "object" && error !== null && "message" in error) {
      const message = (error as { message?: unknown }).message;
      if (typeof message === "string" && message.trim().length > 0) {
        return message;
      }
    }

    if (typeof error === "string" && error.trim().length > 0) {
      return error;
    }

    return "Session state could not be saved.";
  }
}
