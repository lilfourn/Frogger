export type FileAccessStatus = "granted" | "denied";
export type AppearanceMode = "system" | "light" | "dark";
export type ViewMode = "list" | "grid" | "column" | "gallery";
export type SortKey = "name" | "dateModified" | "size" | "kind" | "path";
export type SortDirection = "asc" | "desc";
export type SidebarSectionId = "recents" | "favorites" | "locations";
export type SidebarItemType = "recent" | "favorite" | "drive" | "cloudFolder" | "home";
export type CloudState = "local" | "cloudAvailableOffline" | "cloudOnly" | "unknown";
export type SearchMatchReason = "exact" | "prefix" | "substring" | "fuzzy";
export type IndexingStatus = "notStarted" | "initialBuild" | "reconciling" | "ready" | "failed";
export type FileOperationKind =
  | "newFolder"
  | "rename"
  | "moveToTrash"
  | "copy"
  | "move"
  | "open"
  | "openWith"
  | "indexing"
  | "preview";
export type OperationStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled";
export type PreviewRenderer = "image" | "video" | "audio" | "text" | "pdf" | "fallbackMetadata";

export interface CommandError {
  code: string;
  message: string;
  recoverable: boolean;
  details: string | null;
}

export interface AppBootstrap {
  appVersion: string;
  platform: PlatformInfo;
  access: FileAccessState;
  settings: AppSettings;
  windows: WindowState[];
  sidebar: SidebarState;
  indexing: IndexingState;
  capabilities: AppCapabilities;
  events: EventNames;
}

export interface PlatformInfo {
  os: string;
  family: string;
  pathSeparator: string;
  homeDir: string | null;
}

export interface FileAccessState {
  status: FileAccessStatus;
  homeDir: string | null;
  message: string | null;
  recoveryHint: string | null;
}

export interface AppSettings {
  appearanceMode: AppearanceMode;
  hiddenFilesVisible: boolean;
  fileExtensionsVisible: boolean;
  foldersFirst: boolean;
  pathBarVisible: boolean;
  restoreEnabled: boolean;
  localOnlyIndexing: boolean;
  previewsEnabled: boolean;
  listColumnVisibility: Record<string, boolean>;
  listColumnWidths: Record<string, number>;
  raw: Record<string, string>;
}

export interface WindowState {
  id: string;
  label: string;
  geometry: WindowGeometry;
  activeTabId: string | null;
  tabs: TabState[];
  sidebarWidth: number;
  sidebarCollapsed: boolean;
}

export interface WindowGeometry {
  x: number | null;
  y: number | null;
  width: number;
  height: number;
  fullscreen: boolean;
  maximized: boolean;
}

export interface TabState {
  id: string;
  path: string;
  title: string;
  position: number;
  isActive: boolean;
  folderState: FolderViewState;
}

export interface FolderViewState {
  viewMode: ViewMode;
  sort: SortState;
  foldersFirst: boolean;
  hiddenFilesVisible: boolean;
  fileExtensionsVisible: boolean;
  scrollOffset: number;
  selectedItemPath: string | null;
}

export interface SortState {
  key: SortKey;
  direction: SortDirection;
}

export interface SidebarState {
  sections: SidebarSectionState[];
  recentItems: SidebarItem[];
  favorites: SidebarItem[];
  locations: SidebarItem[];
  recentsVirtualFolderId: string;
}

export interface SidebarSectionState {
  id: SidebarSectionId;
  label: string;
  visible: boolean;
  position: number;
}

export interface SidebarItem {
  id: string;
  label: string;
  path: string;
  itemType: SidebarItemType;
}

export interface DirectoryListRequest {
  path: string;
  sort: SortState;
  foldersFirst: boolean;
  hiddenFilesVisible: boolean;
  fileExtensionsVisible: boolean;
  cursor: string | null;
  limit: number | null;
}

export interface FileEntry {
  path: string;
  parentPath: string;
  name: string;
  displayName: string;
  kind: string;
  isDir: boolean;
  size: number | null;
  modifiedAt: string | null;
  createdAt: string | null;
  hidden: boolean;
  extension: string | null;
  readOnly: boolean;
  icon: FileIcon;
  cloud: CloudState;
  /** True when the entry itself is a symbolic link (alias on macOS). */
  isSymlink?: boolean;
  /** True when the entry is a symlink whose target cannot be resolved. */
  symlinkBroken?: boolean;
  /** Resolved target path of a working symlink, when readable. */
  symlinkTarget?: string | null;
}

export interface FileIcon {
  name: string;
  color: string | null;
}

export interface DirectoryListing {
  path: string;
  entries: FileEntry[];
  totalCount: number;
  nextCursor: string | null;
  loadingComplete: boolean;
}

export interface ThumbnailDescriptor {
  sourcePath: string;
  thumbnailPath: string;
  width: number;
  height: number;
  sourceModifiedAt: string | null;
  sourceSize: number;
  cacheHit: boolean;
}

export interface SearchResult {
  path: string;
  parentPath: string;
  name: string;
  displayName: string;
  kind: string;
  isDir: boolean;
  size: number | null;
  modifiedAt: string | null;
  rank: number;
  matchReason: SearchMatchReason;
}

export interface IndexingState {
  status: IndexingStatus;
  hasInitialIndex: boolean;
  indexedItemCount: number;
  message: string | null;
}

export interface OperationActivity {
  id: string;
  operation: FileOperationKind;
  status: OperationStatus;
  primaryPath: string | null;
  message: string;
  progress: OperationProgress | null;
  recoverable: boolean;
}

export interface OperationProgress {
  completed: number;
  total: number | null;
}

export interface PreviewDescriptor {
  path: string;
  renderer: PreviewRenderer;
  displayName: string;
  kind: string;
  size: number | null;
  metadata: Record<string, string>;
}

export interface AppCapabilities {
  nativeTitlebarTabs: boolean;
  openWithChooser: boolean;
  reliableTrashUndo: boolean;
  outboundFileDrag: boolean;
  cloudPlaceholderDetection: boolean;
}

export interface EventNames {
  directoryListingProgress: string;
  indexingProgress: string;
  fileOperationProgress: string;
  watcherUpdate: string;
  settingsChanged: string;
  activityFailure: string;
}
