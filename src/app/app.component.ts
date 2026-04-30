import { ScrollingModule } from "@angular/cdk/scrolling";
import { Component, OnDestroy, OnInit, computed, effect, inject, isDevMode, signal } from "@angular/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { FroggerApiService } from "./core/frogger-api.service";
import { FroggerEventsService } from "./core/frogger-events.service";
import { SessionStoreService } from "./core/session-store.service";
import type {
  AppBootstrap,
  AppSettings,
  DirectoryListing,
  FileEntry,
  FolderViewState,
  SearchResult,
  SidebarItem,
  SidebarSectionId,
  SidebarState,
  SortDirection,
  SortKey,
  SortState,
  ViewMode,
} from "./core/frogger-api.types";

interface BreadcrumbSegment {
  label: string;
  path: string;
}

interface SidebarNavItem {
  label: string;
  path: string;
  icon: string;
}

type ListColumnId = "name" | "dateModified" | "size" | "kind";

interface ListColumn {
  id: ListColumnId;
  label: string;
  sortKey: SortKey;
  minWidth: number;
}

@Component({
  selector: "app-root",
  imports: [ScrollingModule],
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.css",
})
export class AppComponent implements OnInit, OnDestroy {
  private readonly api = inject(FroggerApiService);
  private readonly events = inject(FroggerEventsService);
  readonly session = inject(SessionStoreService);

  readonly bootstrap = signal<AppBootstrap | null>(null);
  readonly loading = signal(true);
  readonly errorMessage = signal<string | null>(null);
  readonly sidebarCollapsed = signal(false);
  readonly sidebarWidth = signal(236);
  readonly directoryListing = signal<DirectoryListing | null>(null);
  readonly columnListings = signal<DirectoryListing[]>([]);
  readonly columnLoadingPath = signal<string | null>(null);
  readonly listingLoading = signal(false);
  readonly listingError = signal<string | null>(null);
  readonly listingErrorDetails = signal<string | null>(null);
  readonly selectedPath = signal<string | null>(null);
  readonly searchQuery = signal("");
  readonly searchResults = signal<FileEntry[]>([]);
  readonly searchLoading = signal(false);
  readonly searchError = signal<string | null>(null);
  readonly thumbnails = signal<Record<string, string>>({});
  readonly placeholderRows = Array.from({ length: 20 }, (_, index) => index);

  private listingRequestId = 0;
  private navigationRequestId = 0;
  private columnListingRequestId = 0;
  private searchRequestId = 0;
  private activeDirectoryKey: string | null = null;
  private eventUnlisteners: UnlistenFn[] = [];
  private searchDebounceHandle: ReturnType<typeof setTimeout> | null = null;
  private resizingSidebar = false;
  private readonly minSidebarWidth = 180;
  private readonly maxSidebarWidth = 360;
  private readonly thumbnailConcurrency = 6;
  private resizingColumn: ListColumnId | null = null;
  private readonly columns: ListColumn[] = [
    { id: "name", label: "Name", sortKey: "name", minWidth: 180 },
    { id: "size", label: "Size", sortKey: "size", minWidth: 78 },
    { id: "kind", label: "Kind", sortKey: "kind", minWidth: 96 },
    { id: "dateModified", label: "Date Modified", sortKey: "dateModified", minWidth: 150 },
  ];
  private readonly moveSidebarResize = (event: PointerEvent): void => {
    if (!this.resizingSidebar) {
      return;
    }

    this.sidebarWidth.set(this.clampSidebarWidth(event.clientX));
  };
  private readonly stopSidebarResize = (): void => {
    if (!this.resizingSidebar) {
      return;
    }

    this.resizingSidebar = false;
    document.body.classList.remove("is-resizing-sidebar");
    window.removeEventListener("pointermove", this.moveSidebarResize);
    window.removeEventListener("pointerup", this.stopSidebarResize);
    this.session.updateSidebarWidth(this.sidebarWidth());
  };
  private readonly moveColumnResize = (event: PointerEvent): void => {
    if (!this.resizingColumn) {
      return;
    }

    const header = document.querySelector<HTMLElement>(`[data-column-id="${this.resizingColumn}"]`);
    if (!header) {
      return;
    }

    const left = header.getBoundingClientRect().left;
    const column = this.columns.find((candidate) => candidate.id === this.resizingColumn);
    const minWidth = column?.minWidth ?? 72;
    this.setColumnWidth(this.resizingColumn, Math.max(minWidth, Math.round(event.clientX - left)));
  };
  private readonly stopColumnResize = (): void => {
    const columnId = this.resizingColumn;
    if (!columnId) {
      return;
    }

    this.resizingColumn = null;
    document.body.classList.remove("is-resizing-column");
    window.removeEventListener("pointermove", this.moveColumnResize);
    window.removeEventListener("pointerup", this.stopColumnResize);
    void this.persistDisplaySetting(`list.column.${columnId}.width`, `${this.columnWidth(columnId)}`);
  };

  readonly viewModes: { id: ViewMode; label: string; icon: string }[] = [
    { id: "grid", label: "Icon View", icon: "icon-view-grid" },
    { id: "list", label: "List View", icon: "icon-view-list" },
    { id: "column", label: "Column View", icon: "icon-view-column" },
    { id: "gallery", label: "Gallery View", icon: "icon-view-gallery" },
  ];

  readonly isSearchActive = computed(() => this.searchQuery().trim().length > 0);
  readonly selectedCount = computed(() => (this.selectedPath() ? 1 : 0));
  readonly selectedEntry = computed(() => {
    const selectedPath = this.selectedPath();
    if (!selectedPath) {
      return null;
    }

    const entries = this.isSearchActive()
      ? this.searchResults()
      : this.directoryListing()?.entries ?? [];
    return entries.find((entry) => entry.path === selectedPath) ?? null;
  });
  readonly favoriteTargetPath = computed(() => {
    const selectedEntry = this.selectedEntry();
    if (selectedEntry?.isDir) {
      return selectedEntry.path;
    }

    const activePath = this.session.activeTab()?.path ?? null;
    return activePath && !this.isRecentsPath(activePath) ? activePath : null;
  });
  readonly galleryPreviewEntry = computed(() => {
    const selected = this.selectedEntry();
    if (selected) {
      return selected;
    }

    return this.directoryListing()?.entries[0] ?? null;
  });
  readonly favoriteTargetPinned = computed(() => {
    const targetPath = this.favoriteTargetPath();
    if (!targetPath) {
      return false;
    }

    return this.bootstrap()?.sidebar.favorites.some(
      (favorite) => this.normalizePath(favorite.path) === this.normalizePath(targetPath),
    ) ?? false;
  });

  readonly breadcrumbs = computed<BreadcrumbSegment[]>(() => {
    const path = this.session.activeTab()?.path;
    if (!path) {
      return [];
    }

    return this.toBreadcrumbs(path);
  });

  constructor() {
    effect(() => {
      const activeTab = this.session.activeTab();
      if (!activeTab) {
        this.activeDirectoryKey = null;
        this.directoryListing.set(null);
        this.columnListings.set([]);
        this.listingError.set(null);
        this.listingErrorDetails.set(null);
        this.listingLoading.set(false);
        this.selectedPath.set(null);
        this.thumbnails.set({});
        return;
      }

      const { sort, foldersFirst, hiddenFilesVisible, fileExtensionsVisible } = activeTab.folderState;
      const directoryKey = this.directoryKey(
        activeTab.path,
        sort,
        foldersFirst,
        hiddenFilesVisible,
        fileExtensionsVisible,
      );
      if (directoryKey === this.activeDirectoryKey) {
        return;
      }

      this.activeDirectoryKey = directoryKey;
      void this.loadDirectory(
        activeTab.path,
        sort,
        foldersFirst,
        hiddenFilesVisible,
        fileExtensionsVisible,
      );
    });
  }

  ngOnInit(): void {
    void this.loadBootstrap();
  }

  ngOnDestroy(): void {
    this.clearEventListeners();
    if (this.searchDebounceHandle) {
      clearTimeout(this.searchDebounceHandle);
      this.searchDebounceHandle = null;
    }
  }

  retryBootstrap(): void {
    void this.loadBootstrap();
  }

  onSearchInput(event: Event): void {
    const value = event.target instanceof HTMLInputElement ? event.target.value : "";
    this.searchQuery.set(value);
    this.selectedPath.set(null);
    this.searchError.set(null);

    if (this.searchDebounceHandle) {
      clearTimeout(this.searchDebounceHandle);
      this.searchDebounceHandle = null;
    }

    const query = value.trim();
    if (!query) {
      this.searchRequestId += 1;
      this.searchResults.set([]);
      this.searchLoading.set(false);
      return;
    }

    this.searchLoading.set(true);
    const requestId = ++this.searchRequestId;
    this.searchDebounceHandle = setTimeout(() => {
      this.searchDebounceHandle = null;
      void this.runSearch(query, requestId);
    }, 125);
  }

  clearSearch(): void {
    if (this.searchDebounceHandle) {
      clearTimeout(this.searchDebounceHandle);
      this.searchDebounceHandle = null;
    }
    this.searchRequestId += 1;
    this.searchQuery.set("");
    this.searchResults.set([]);
    this.searchLoading.set(false);
    this.searchError.set(null);
    this.selectedPath.set(null);
  }

  openHomeTab(): void {
    const activeWindow = this.session.activeWindow();
    const homePath = this.bootstrap()?.access.homeDir;
    if (activeWindow && homePath) {
      this.session.openTab(activeWindow.id, homePath, "Home");
    }
  }

  closeActiveTab(): void {
    const activeWindow = this.session.activeWindow();
    const activeTab = this.session.activeTab();
    if (activeWindow && activeTab) {
      this.session.closeTab(activeWindow.id, activeTab.id);
    }
  }

  switchTab(tabId: string): void {
    const activeWindow = this.session.activeWindow();
    if (activeWindow) {
      this.session.switchTab(activeWindow.id, tabId);
    }
  }

  createWindow(): void {
    void this.session.createWindow(this.bootstrap()?.access.homeDir ?? null);
  }

  setViewMode(viewMode: ViewMode): void {
    const activeTab = this.session.activeTab();
    if (!activeTab) {
      return;
    }

    this.updateActiveFolderState({ ...activeTab.folderState, viewMode });
  }

  listColumns(): ListColumn[] {
    return this.columns.filter((column) => this.columnVisible(column.id));
  }

  columnVisible(columnId: ListColumnId): boolean {
    if (columnId === "name") {
      return true;
    }

    return this.bootstrap()?.settings.listColumnVisibility[columnId] ?? true;
  }

  columnWidth(columnId: ListColumnId): number {
    const fallback = this.columns.find((column) => column.id === columnId)?.minWidth ?? 120;
    return this.bootstrap()?.settings.listColumnWidths[columnId] ?? fallback;
  }

  listGridTemplate(): string {
    return this.listColumns()
      .map((column) => column.id === "name" ? `minmax(${column.minWidth}px, 1fr)` : `${this.columnWidth(column.id)}px`)
      .join(" ");
  }

  async sortBy(column: ListColumn): Promise<void> {
    const activeTab = this.session.activeTab();
    if (!activeTab) {
      return;
    }

    const currentSort = activeTab.folderState.sort;
    const direction = currentSort.key === column.sortKey && currentSort.direction === "asc" ? "desc" : "asc";
    this.updateActiveFolderState({
      ...activeTab.folderState,
      sort: { key: column.sortKey, direction },
      scrollOffset: 0,
    });
  }

  sortIndicator(column: ListColumn): string {
    const sort = this.session.activeTab()?.folderState.sort;
    if (sort?.key !== column.sortKey) {
      return "";
    }

    return sort.direction === "asc" ? "↑" : "↓";
  }

  async toggleDisplaySetting(key: "foldersFirst" | "hiddenFilesVisible" | "fileExtensionsVisible"): Promise<void> {
    const activeTab = this.session.activeTab();
    const settings = this.bootstrap()?.settings;
    if (!activeTab || !settings) {
      return;
    }

    const nextValue = !settings[key];
    const settingKey = key === "foldersFirst"
      ? "browser.foldersFirst"
      : key === "hiddenFilesVisible"
        ? "browser.hiddenFilesVisible"
        : "browser.fileExtensionsVisible";
    await this.persistDisplaySetting(settingKey, String(nextValue));
    this.updateActiveFolderState({ ...activeTab.folderState, [key]: nextValue });
  }

  async toggleColumn(columnId: ListColumnId): Promise<void> {
    if (columnId === "name") {
      return;
    }

    await this.persistDisplaySetting(
      `list.column.${columnId}.visible`,
      String(!this.columnVisible(columnId)),
    );
  }

  startColumnResize(event: PointerEvent, columnId: ListColumnId): void {
    event.preventDefault();
    event.stopPropagation();
    this.resizingColumn = columnId;
    document.body.classList.add("is-resizing-column");
    window.addEventListener("pointermove", this.moveColumnResize);
    window.addEventListener("pointerup", this.stopColumnResize);
  }

  retryDirectoryListing(): void {
    const activeTab = this.session.activeTab();
    if (!activeTab) {
      return;
    }

    const { sort, foldersFirst, hiddenFilesVisible, fileExtensionsVisible } = activeTab.folderState;
    void this.loadDirectory(
      activeTab.path,
      sort,
      foldersFirst,
      hiddenFilesVisible,
      fileExtensionsVisible,
    );
  }

  selectEntry(entry: FileEntry): void {
    this.selectedPath.set(entry.path);

    const activeTab = this.session.activeTab();
    if (activeTab && !this.isSearchActive()) {
      this.updateActiveFolderState({ ...activeTab.folderState, selectedItemPath: entry.path });
    }
  }

  async selectColumnEntry(entry: FileEntry, columnIndex: number): Promise<void> {
    const requestId = ++this.columnListingRequestId;
    this.selectEntry(entry);
    if (!entry.isDir) {
      this.columnListings.update((columns) => columns.slice(0, columnIndex + 1));
      this.columnLoadingPath.set(null);
      return;
    }

    const activeTab = this.session.activeTab();
    const activeWindowId = this.session.activeWindow()?.id ?? null;
    const activeTabId = activeTab?.id ?? null;
    if (!activeTab) {
      return;
    }

    this.columnLoadingPath.set(entry.path);
    try {
      const listing = await this.api.listDirectory(
        entry.path,
        activeTab.folderState.sort,
        activeTab.folderState.foldersFirst,
        activeTab.folderState.hiddenFilesVisible,
        activeTab.folderState.fileExtensionsVisible,
      );
      if (
        requestId === this.columnListingRequestId &&
        this.selectedPath() === entry.path &&
        this.session.activeWindow()?.id === activeWindowId &&
        this.session.activeTab()?.id === activeTabId
      ) {
        this.columnListings.update((columns) => [...columns.slice(0, columnIndex + 1), listing]);
      }
    } catch (error: unknown) {
      if (requestId === this.columnListingRequestId) {
        this.listingError.set(this.toErrorMessage(error));
      }
    } finally {
      if (requestId === this.columnListingRequestId) {
        this.columnLoadingPath.set(null);
      }
    }
  }

  async openEntry(entry: FileEntry): Promise<void> {
    // Broken aliases have an unreachable target. Opening them with the
    // default app or navigating into them will always fail with ENOENT —
    // show a clear, contained error rather than letting the call bubble up
    // as a whole-folder listing failure.
    if (entry.symlinkBroken) {
      const message = `“${entry.displayName}” is a broken alias and cannot be opened.`;
      if (this.isSearchActive()) {
        this.searchError.set(message);
      } else {
        this.listingError.set(message);
        this.listingErrorDetails.set(
          entry.symlinkTarget ? `missing target — ${entry.symlinkTarget}` : null,
        );
      }
      return;
    }

    if (entry.isDir) {
      await this.navigateToPath(entry.path, entry.displayName);
      void this.recordRecent(entry.path);
      return;
    }

    try {
      const sidebar = await this.api.openFileWithDefaultApp(entry.path);
      this.applySidebarState(sidebar);
    } catch (error: unknown) {
      const message = this.toErrorMessage(error);
      if (this.isSearchActive()) {
        this.searchError.set(message);
      } else {
        this.listingError.set(message);
      }
    }
  }

  async toggleFavoriteTarget(): Promise<void> {
    const targetPath = this.favoriteTargetPath();
    if (!targetPath) {
      return;
    }

    try {
      const sidebar = this.favoriteTargetPinned()
        ? await this.api.unpinSidebarFolder(targetPath)
        : await this.api.pinSidebarFolder(targetPath, this.folderName(targetPath));
      this.applySidebarState(sidebar);
    } catch (error: unknown) {
      this.listingError.set(this.toErrorMessage(error));
    }
  }

  trackEntry(_index: number, entry: FileEntry): string {
    return entry.path;
  }

  thumbnailSrc(entry: FileEntry): string | null {
    return this.thumbnails()[entry.path] ?? null;
  }

  fileGlyphClass(entry: FileEntry, extraClass: string | null = null): string {
    const classes = extraClass ? [extraClass, "file-glyph"] : ["file-glyph"];

    if (entry.isDir) {
      classes.push("file-glyph--folder");
    } else {
      const assetClass = this.fileIconAssetClass(entry.icon.name);
      if (assetClass) {
        classes.push("file-glyph--asset", assetClass);
      }
    }

    if (entry.isSymlink) {
      classes.push("file-glyph--alias");
    }

    return classes.join(" ");
  }

  symlinkTooltip(entry: FileEntry): string | null {
    if (entry.symlinkBroken) {
      return entry.symlinkTarget
        ? `Broken alias → ${entry.symlinkTarget}`
        : "Broken alias (target cannot be resolved)";
    }
    if (entry.isSymlink && entry.symlinkTarget) {
      return `Alias → ${entry.symlinkTarget}`;
    }
    return null;
  }

  navigateToBreadcrumb(path: string): void {
    void this.navigateToPath(path, this.folderName(path));
  }

  openSidebarPath(path: string, label: string): void {
    void this.navigateToPath(path, label);
  }

  toggleSidebar(): void {
    this.sidebarCollapsed.update((collapsed) => {
      const next = !collapsed;
      this.session.updateSidebarCollapsed(next);
      return next;
    });
  }

  startSidebarResize(event: PointerEvent): void {
    if (this.sidebarCollapsed()) {
      return;
    }

    event.preventDefault();
    this.resizingSidebar = true;
    document.body.classList.add("is-resizing-sidebar");
    window.addEventListener("pointermove", this.moveSidebarResize);
    window.addEventListener("pointerup", this.stopSidebarResize);
  }

  sidebarSectionVisible(state: AppBootstrap, sectionId: SidebarSectionId): boolean {
    return state.sidebar.sections.find((section) => section.id === sectionId)?.visible ?? true;
  }

  async setSidebarSectionVisibility(sectionId: SidebarSectionId, visible: boolean): Promise<void> {
    try {
      const sidebar = await this.api.setSidebarSectionVisibility(sectionId, visible);
      this.applySidebarState(sidebar);
    } catch (error: unknown) {
      this.listingError.set(this.toErrorMessage(error));
    }
  }

  isActivePath(path: string): boolean {
    return this.normalizePath(this.session.activeTab()?.path) === this.normalizePath(path);
  }

  recentSidebarItems(state: AppBootstrap): SidebarNavItem[] {
    return [
      {
        label: "Recents",
        path: state.sidebar.recentsVirtualFolderId,
        icon: "icon-recents",
      },
    ];
  }

  favoriteSidebarItems(state: AppBootstrap): SidebarNavItem[] {
    const home = state.access.homeDir;
    const defaults: SidebarNavItem[] = home
      ? [
          { label: this.folderName(home), path: home, icon: "icon-home" },
          { label: "Desktop", path: this.joinPath(home, "Desktop"), icon: "icon-desktop" },
          { label: "Downloads", path: this.joinPath(home, "Downloads"), icon: "icon-downloads" },
          { label: "Documents", path: this.joinPath(home, "Documents"), icon: "icon-documents" },
          { label: "Applications", path: "/Applications", icon: "icon-applications" },
        ]
      : [];

    const pinned = state.sidebar.favorites.map((favorite) => this.toSidebarNavItem(favorite));
    return this.uniqueSidebarItems([...pinned, ...defaults]);
  }

  locationSidebarItems(state: AppBootstrap): SidebarNavItem[] {
    const home = state.access.homeDir;
    // NOTE: AirDrop is intentionally omitted here. On macOS, AirDrop is not a
    // filesystem path — it is a Finder sharing UI backed by NSSharingService.
    // Previously it was modelled with `path: home`, which caused it to collide
    // with the Favorites "Home" entry so clicking Home highlighted both items
    // (and clicking AirDrop silently navigated to Home). Re-introduce only
    // once a real virtual action (e.g. open Finder's AirDrop.app) is wired up.
    const defaults: SidebarNavItem[] = home
      ? [
          { label: "iCloud Drive", path: this.joinPath(home, "Library/Mobile Documents/com~apple~CloudDocs"), icon: "icon-cloud" },
          { label: "Network", path: "/Network", icon: "icon-network" },
        ]
      : [];

    const detected = state.sidebar.locations
      .filter((location) => location.itemType !== "home")
      .map((location) => this.toSidebarNavItem(location));

    // Cross-section dedup: don't show a Location whose path is already covered
    // by the Favorites section (e.g. Home). This keeps sidebar selection
    // unambiguous since `isActivePath` compares by normalized path.
    const favoritePaths = new Set(
      this.favoriteSidebarItems(state).map((item) => this.normalizePath(item.path)),
    );
    const merged = [...detected, ...defaults].filter(
      (item) => !favoritePaths.has(this.normalizePath(item.path)),
    );
    return this.uniqueSidebarItems(merged);
  }

  folderName(path: string | null | undefined): string {
    if (!path) {
      return "Home";
    }

    if (this.isRecentsPath(path)) {
      return "Recents";
    }

    const normalized = path.replace(/[/\\]+$/, "");
    const parts = normalized.split(/[/\\]/).filter(Boolean);
    return parts.at(-1) ?? "Home";
  }

  searchPlaceholder(state: AppBootstrap): string {
    return state.indexing.hasInitialIndex ? "Search" : "Indexing…";
  }

  statusLabel(state: AppBootstrap): string {
    if (!state.indexing.hasInitialIndex) {
      return "Indexing";
    }

    const selectedCount = this.selectedCount();

    if (this.isSearchActive()) {
      if (this.searchLoading()) {
        return "Searching";
      }

      const count = this.searchResults().length;
      const resultLabel = count === 1 ? "1 result" : `${count} results`;
      return selectedCount === 0 ? resultLabel : `${resultLabel}, ${selectedCount} selected`;
    }

    const listing = this.directoryListing();

    if (this.listingLoading()) {
      return "Loading";
    }

    if (!listing) {
      return "0 items";
    }

    const itemLabel = listing.totalCount === 1 ? "1 item" : `${listing.totalCount} items`;
    if (selectedCount === 0) {
      return itemLabel;
    }

    return `${itemLabel}, ${selectedCount} selected`;
  }

  fileSizeLabel(entry: FileEntry): string {
    if (entry.isDir || entry.size === null) {
      return "—";
    }

    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = entry.size;
    let unitIndex = 0;
    while (value >= 1024 && unitIndex < units.length - 1) {
      value /= 1024;
      unitIndex += 1;
    }

    return unitIndex === 0 ? `${value} ${units[unitIndex]}` : `${value.toFixed(1)} ${units[unitIndex]}`;
  }

  dateLabel(value: string | null): string {
    if (!value) {
      return "—";
    }

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return "—";
    }

    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      year: date.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(date);
  }

  currentSortKey(): SortKey {
    return this.session.activeTab()?.folderState.sort.key ?? "name";
  }

  currentSortDirection(): SortDirection {
    return this.session.activeTab()?.folderState.sort.direction ?? "asc";
  }

  sortLabel(): string {
    const labels: Record<SortKey, string> = {
      name: "Name",
      dateModified: "Date Modified",
      size: "Size",
      kind: "Kind",
      path: "Path",
    };

    const direction = this.currentSortDirection() === "asc" ? "Ascending" : "Descending";
    return `${labels[this.currentSortKey()]} · ${direction}`;
  }

  setSortKey(value: string): void {
    if (!this.isSortKey(value)) {
      return;
    }

    const activeTab = this.session.activeTab();
    if (!activeTab) {
      return;
    }

    this.updateActiveFolderState({
      ...activeTab.folderState,
      sort: { ...activeTab.folderState.sort, key: value },
      scrollOffset: 0,
    });
  }

  toggleSortDirection(): void {
    const activeTab = this.session.activeTab();
    if (!activeTab) {
      return;
    }

    this.updateActiveFolderState({
      ...activeTab.folderState,
      sort: {
        ...activeTab.folderState.sort,
        direction: activeTab.folderState.sort.direction === "asc" ? "desc" : "asc",
      },
      scrollOffset: 0,
    });
  }

  private async navigateToPath(path: string, label: string): Promise<void> {
    const requestId = ++this.navigationRequestId;
    const activeWindowId = this.session.activeWindow()?.id ?? null;
    const activeTabId = this.session.activeTab()?.id ?? null;
    let folderState: FolderViewState | null = null;

    try {
      folderState = this.isRecentsPath(path) ? null : await this.api.getFolderViewState(path);
    } catch {
      folderState = null;
    }

    if (
      requestId !== this.navigationRequestId ||
      this.session.activeWindow()?.id !== activeWindowId ||
      this.session.activeTab()?.id !== activeTabId
    ) {
      return;
    }

    if (this.isSearchActive()) {
      this.clearSearch();
    }
    this.session.updateActiveTabPath(path, label, folderState);
  }

  private updateActiveFolderState(folderState: FolderViewState): void {
    this.session.updateActiveTabFolderState(folderState);
  }

  private async persistDisplaySetting(key: string, value: string): Promise<void> {
    try {
      const settings = await this.api.setBrowserDisplaySetting(key, value);
      this.applySettings(settings);
    } catch (error: unknown) {
      this.listingError.set(this.toErrorMessage(error));
    }
  }

  private applySettings(settings: AppSettings): void {
    this.bootstrap.update((bootstrap) => (bootstrap ? { ...bootstrap, settings } : bootstrap));
    this.session.updateSettings(settings);
  }

  private setColumnWidth(columnId: ListColumnId, width: number): void {
    const settings = this.bootstrap()?.settings;
    if (!settings) {
      return;
    }

    this.applySettings({
      ...settings,
      listColumnWidths: {
        ...settings.listColumnWidths,
        [columnId]: width,
      },
    });
  }

  private async loadBootstrap(): Promise<void> {
    this.loading.set(true);
    this.errorMessage.set(null);

    try {
      const bootstrap = await this.api.bootstrap();
      this.bootstrap.set(bootstrap);
      this.session.initialize(bootstrap);
      const activeWindow = this.session.activeWindow();
      this.sidebarCollapsed.set(activeWindow?.sidebarCollapsed ?? false);
      this.sidebarWidth.set(this.clampSidebarWidth(activeWindow?.sidebarWidth ?? 236));
      void this.registerBootstrapEvents(bootstrap);
    } catch (error: unknown) {
      this.errorMessage.set(this.toErrorMessage(error));
    } finally {
      this.loading.set(false);
    }
  }

  private clearEventListeners(): void {
    for (const unlisten of this.eventUnlisteners) {
      unlisten();
    }
    this.eventUnlisteners = [];
  }

  private async runSearch(query: string, requestId: number): Promise<void> {
    if (!this.bootstrap()?.indexing.hasInitialIndex) {
      this.searchLoading.set(false);
      return;
    }

    try {
      const results = await this.api.searchMetadata(query, 100);
      if (requestId === this.searchRequestId && this.searchQuery().trim() === query) {
        this.searchResults.set(results.map((result) => this.searchResultToFileEntry(result)));
        this.searchError.set(null);
      }
    } catch (error: unknown) {
      if (requestId === this.searchRequestId) {
        this.searchResults.set([]);
        this.searchError.set(this.toErrorMessage(error));
      }
    } finally {
      if (requestId === this.searchRequestId) {
        this.searchLoading.set(false);
      }
    }
  }

  private searchResultToFileEntry(result: SearchResult): FileEntry {
    const extension = result.isDir ? null : this.extensionForName(result.name);
    return {
      path: result.path,
      parentPath: result.parentPath,
      name: result.name,
      displayName: result.displayName,
      kind: result.kind,
      isDir: result.isDir,
      size: result.size,
      modifiedAt: result.modifiedAt,
      createdAt: null,
      hidden: result.name.startsWith("."),
      extension,
      readOnly: false,
      icon: this.iconForSearchResult(result, extension),
      cloud: "local",
      isSymlink: result.kind.toLowerCase().includes("alias"),
      symlinkBroken: false,
      symlinkTarget: null,
    };
  }

  private async registerBootstrapEvents(bootstrap: AppBootstrap): Promise<void> {
    this.clearEventListeners();

    try {
      this.eventUnlisteners = await this.events.listenToBootstrapEvents(bootstrap.events, {
        indexingProgress: (indexing) => {
          this.bootstrap.update((current) => current ? { ...current, indexing } : current);
        },
      });
    } catch (error: unknown) {
      if (isDevMode()) {
        // eslint-disable-next-line no-console
        console.warn("[frogger] failed to register backend event listeners", error);
      }
    }
  }

  private async loadDirectory(
    path: string,
    sort: SortState,
    foldersFirst: boolean,
    hiddenFilesVisible: boolean,
    fileExtensionsVisible: boolean,
  ): Promise<void> {
    const requestId = ++this.listingRequestId;
    this.listingLoading.set(true);
    this.listingError.set(null);
    this.listingErrorDetails.set(null);
    this.selectedPath.set(null);
    this.thumbnails.set({});

    // Diagnostic (dev-only): log the exact path string being sent to the Tauri
    // backend so we can distinguish path-mangling bugs from real fs errors
    // when the UI reports "Folder unavailable".
    if (isDevMode()) {
      // eslint-disable-next-line no-console
      console.log("[frogger] list_directory request", {
        path,
        length: path.length,
        codepoints: Array.from(path).map((ch) => ch.codePointAt(0)),
      });
    }

    try {
      const listing = await this.api.listDirectory(
        path,
        sort,
        foldersFirst,
        hiddenFilesVisible,
        fileExtensionsVisible,
      );
      if (requestId === this.listingRequestId) {
        const activeTab = this.session.activeTab();
        const selectedItemPath = activeTab?.path === listing.path ? activeTab.folderState.selectedItemPath : null;
        this.directoryListing.set(listing);
        this.columnListings.set([listing]);
        this.selectedPath.set(
          selectedItemPath && listing.entries.some((entry) => entry.path === selectedItemPath)
            ? selectedItemPath
            : null,
        );
        void this.loadThumbnails(listing.entries, requestId, listing.path);
      }
    } catch (error: unknown) {
      if (requestId === this.listingRequestId) {
        // Diagnostic (dev-only): log the full error object so `code` and
        // `details` are visible in the dev console even though the UI only
        // shows `message`.
        if (isDevMode()) {
          // eslint-disable-next-line no-console
          console.error("[frogger] list_directory failed", { path, error });
        }
        this.directoryListing.set(null);
        this.columnListings.set([]);
        this.listingError.set(this.toErrorMessage(error));
        this.listingErrorDetails.set(this.toErrorDetails(error));
      }
    } finally {
      if (requestId === this.listingRequestId) {
        this.listingLoading.set(false);
      }
    }
  }

  private async loadThumbnails(entries: FileEntry[], requestId: number, listingPath: string): Promise<void> {
    const imageEntries = entries.filter((entry) => this.isThumbnailCandidate(entry)).slice(0, 160);
    if (imageEntries.length === 0) {
      if (requestId === this.listingRequestId && this.directoryListing()?.path === listingPath) {
        this.thumbnails.set({});
      }
      return;
    }

    const next: Record<string, string> = {};
    let nextIndex = 0;
    const workerCount = Math.min(this.thumbnailConcurrency, imageEntries.length);
    const workers = Array.from({ length: workerCount }, async () => {
      while (nextIndex < imageEntries.length) {
        if (requestId !== this.listingRequestId || this.directoryListing()?.path !== listingPath) {
          return;
        }

        const entry = imageEntries[nextIndex++];
        try {
          const thumbnail = await this.api.getThumbnail(entry.path);
          if (thumbnail) {
            next[entry.path] = convertFileSrc(thumbnail.thumbnailPath);
          }
        } catch {
          // Thumbnail failures should not block browsing.
        }
      }
    });
    await Promise.all(workers);

    if (requestId === this.listingRequestId && this.directoryListing()?.path === listingPath) {
      this.thumbnails.set(next);
    }
  }

  private isThumbnailCandidate(entry: FileEntry): boolean {
    if (entry.isDir || entry.cloud === "cloudOnly") {
      return false;
    }

    return ["png", "jpg", "jpeg", "webp"].includes((entry.extension ?? "").toLowerCase());
  }

  private extensionForName(name: string): string | null {
    const index = name.lastIndexOf(".");
    if (index <= 0 || index === name.length - 1) {
      return null;
    }

    return name.slice(index + 1).toLowerCase();
  }

  private iconForSearchResult(result: SearchResult, extension: string | null): FileEntry["icon"] {
    if (result.isDir) {
      return { name: "folder", color: "blue" };
    }

    const kind = result.kind.toLowerCase();
    if (kind.includes("spreadsheet")) {
      return { name: "spreadsheet", color: null };
    }
    if (kind.includes("pdf")) {
      return { name: "pdf", color: null };
    }
    if (kind.includes("word")) {
      return { name: "word-document", color: null };
    }
    if (kind.includes("markdown")) {
      return { name: "markdown", color: null };
    }
    if (kind.includes("archive")) {
      return { name: "archive", color: null };
    }
    if (extension && ["png", "jpg", "jpeg", "webp"].includes(extension)) {
      return { name: "file", color: null };
    }

    return { name: "generic", color: null };
  }

  private fileIconAssetClass(name: string): string | null {
    switch (name) {
      case "spreadsheet":
        return "file-glyph--spreadsheet";
      case "pdf":
        return "file-glyph--pdf";
      case "word-document":
        return "file-glyph--document";
      case "markdown":
        return "file-glyph--markdown";
      case "archive":
        return "file-glyph--archive";
      case "document":
      case "generic":
        return "file-glyph--generic";
      default:
        return null;
    }
  }

  private async recordRecent(path: string): Promise<void> {
    try {
      const sidebar = await this.api.recordRecentItem(path);
      this.applySidebarState(sidebar);
    } catch {
      // Browsing should not be interrupted if the recents list cannot be updated.
    }
  }

  private applySidebarState(sidebar: SidebarState): void {
    this.bootstrap.update((bootstrap) => (bootstrap ? { ...bootstrap, sidebar } : bootstrap));
    this.session.updateSidebarState(sidebar);
  }

  private toSidebarNavItem(item: SidebarItem): SidebarNavItem {
    const icon =
      item.itemType === "cloudFolder"
        ? "icon-cloud"
        : item.itemType === "drive"
          ? "icon-drive"
          : item.itemType === "home"
            ? "icon-home"
            : "icon-folder";

    return {
      label: item.label,
      path: item.path,
      icon,
    };
  }

  private uniqueSidebarItems(items: SidebarNavItem[]): SidebarNavItem[] {
    const seen = new Set<string>();
    return items.filter((item) => {
      const key = this.normalizePath(item.path);
      if (seen.has(key)) {
        return false;
      }
      seen.add(key);
      return true;
    });
  }

  private directoryKey(
    path: string,
    sort: SortState,
    foldersFirst: boolean,
    hiddenFilesVisible: boolean,
    fileExtensionsVisible: boolean,
  ): string {
    return JSON.stringify({
      path,
      sortKey: sort.key,
      sortDirection: sort.direction,
      foldersFirst,
      hiddenFilesVisible,
      fileExtensionsVisible,
    });
  }

  private isSortKey(value: string): value is SortKey {
    return ["name", "dateModified", "size", "kind", "path"].includes(value);
  }

  private normalizePath(path: string | null | undefined): string {
    return (path ?? "").replace(/[/\\]+$/, "").toLowerCase();
  }

  private clampSidebarWidth(width: number): number {
    return Math.min(this.maxSidebarWidth, Math.max(this.minSidebarWidth, Math.round(width)));
  }

  private isRecentsPath(path: string): boolean {
    return path === "recents" || path === "frogger://recents";
  }

  private joinPath(base: string, child: string): string {
    const separator = base.includes("\\") ? "\\" : "/";
    return `${base.replace(/[/\\]+$/, "")}${separator}${child.replace(/^[/\\]+/, "")}`;
  }

  private toBreadcrumbs(path: string): BreadcrumbSegment[] {
    if (this.isRecentsPath(path)) {
      return [{ label: "Recents", path }];
    }

    const separator = path.includes("\\") ? "\\" : "/";
    const isAbsoluteUnix = path.startsWith("/");
    const driveMatch = /^[A-Za-z]:/.exec(path);
    const trimmed = path.replace(/[/\\]+$/, "");
    const parts = trimmed.split(/[/\\]/).filter(Boolean);
    const breadcrumbs: BreadcrumbSegment[] = [];

    if (driveMatch) {
      const drive = driveMatch[0];
      breadcrumbs.push({ label: drive, path: `${drive}${separator}` });
      const rest = parts.slice(1);
      let current = `${drive}${separator}`;
      for (const part of rest) {
        current = current.endsWith(separator) ? `${current}${part}` : `${current}${separator}${part}`;
        breadcrumbs.push({ label: part, path: current });
      }
      return breadcrumbs;
    }

    if (isAbsoluteUnix) {
      breadcrumbs.push({ label: "Macintosh HD", path: "/" });
      let current = "";
      for (const part of parts) {
        current = `${current}/${part}`;
        breadcrumbs.push({ label: part, path: current });
      }
      return breadcrumbs;
    }

    let current = "";
    for (const part of parts) {
      current = current ? `${current}${separator}${part}` : part;
      breadcrumbs.push({ label: part, path: current });
    }

    return breadcrumbs;
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

    return "The file manager could not initialize. Retry after checking app and filesystem permissions.";
  }

  private toErrorDetails(error: unknown): string | null {
    if (typeof error === "object" && error !== null) {
      const record = error as { code?: unknown; details?: unknown };
      const parts: string[] = [];
      if (typeof record.code === "string" && record.code.trim().length > 0) {
        parts.push(record.code);
      }
      if (typeof record.details === "string" && record.details.trim().length > 0) {
        parts.push(record.details);
      }
      if (parts.length > 0) {
        return parts.join(" \u2014 ");
      }
    }
    return null;
  }
}
