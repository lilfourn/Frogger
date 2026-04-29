use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppBootstrap {
    pub app_version: String,
    pub platform: PlatformInfo,
    pub access: FileAccessState,
    pub settings: AppSettings,
    pub windows: Vec<WindowState>,
    pub sidebar: SidebarState,
    pub indexing: IndexingState,
    pub capabilities: AppCapabilities,
    pub events: EventNames,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub family: String,
    pub path_separator: String,
    pub home_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileAccessState {
    pub status: FileAccessStatus,
    pub home_dir: Option<String>,
    pub message: Option<String>,
    pub recovery_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileAccessStatus {
    Granted,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub appearance_mode: AppearanceMode,
    pub hidden_files_visible: bool,
    pub file_extensions_visible: bool,
    pub folders_first: bool,
    pub path_bar_visible: bool,
    pub restore_enabled: bool,
    pub local_only_indexing: bool,
    pub previews_enabled: bool,
    pub list_column_visibility: BTreeMap<String, bool>,
    pub list_column_widths: BTreeMap<String, f64>,
    pub raw: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AppearanceMode {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub id: String,
    pub label: String,
    pub geometry: WindowGeometry,
    pub active_tab_id: Option<String>,
    pub tabs: Vec<TabState>,
    pub sidebar_width: f64,
    pub sidebar_collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowGeometry {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: f64,
    pub height: f64,
    pub fullscreen: bool,
    pub maximized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabState {
    pub id: String,
    pub path: String,
    pub title: String,
    pub position: i64,
    pub is_active: bool,
    pub folder_state: FolderViewState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FolderViewState {
    pub view_mode: ViewMode,
    pub sort: SortState,
    pub folders_first: bool,
    pub hidden_files_visible: bool,
    pub file_extensions_visible: bool,
    pub scroll_offset: f64,
    pub selected_item_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    List,
    Grid,
    Column,
    Gallery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SortState {
    pub key: SortKey,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    Name,
    DateModified,
    Size,
    Kind,
    Path,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SidebarState {
    pub sections: Vec<SidebarSectionState>,
    pub recent_items: Vec<SidebarItem>,
    pub favorites: Vec<SidebarItem>,
    pub locations: Vec<SidebarItem>,
    pub recents_virtual_folder_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SidebarSectionState {
    pub id: SidebarSectionId,
    pub label: String,
    pub visible: bool,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SidebarSectionId {
    Recents,
    Favorites,
    Locations,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SidebarItem {
    pub id: String,
    pub label: String,
    pub path: String,
    pub item_type: SidebarItemType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SidebarItemType {
    Recent,
    Favorite,
    Drive,
    CloudFolder,
    Home,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListRequest {
    pub path: String,
    pub sort: SortState,
    pub folders_first: bool,
    pub hidden_files_visible: bool,
    pub file_extensions_visible: bool,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub parent_path: String,
    pub name: String,
    pub display_name: String,
    pub kind: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
    pub created_at: Option<String>,
    pub hidden: bool,
    pub extension: Option<String>,
    pub read_only: bool,
    pub icon: FileIcon,
    pub cloud: CloudState,
    /// True when the entry itself is a symbolic link (alias on macOS).
    #[serde(default)]
    pub is_symlink: bool,
    /// True when the entry is a symlink whose target could not be resolved.
    /// A broken symlink is surfaced to the UI as a non-directory entry with
    /// a visual "broken alias" badge rather than blocking its parent listing.
    #[serde(default)]
    pub symlink_broken: bool,
    /// Resolved target path for a working symlink, if readable. `None` for
    /// non-symlinks and for broken symlinks (use `path` for the link itself).
    #[serde(default)]
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileIcon {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CloudState {
    Local,
    CloudAvailableOffline,
    CloudOnly,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub total_count: usize,
    pub next_cursor: Option<String>,
    pub loading_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailDescriptor {
    pub source_path: String,
    pub thumbnail_path: String,
    pub width: u32,
    pub height: u32,
    pub source_modified_at: Option<String>,
    pub source_size: u64,
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub path: String,
    pub parent_path: String,
    pub name: String,
    pub display_name: String,
    pub kind: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
    pub rank: i64,
    pub match_reason: SearchMatchReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SearchMatchReason {
    Exact,
    Prefix,
    Substring,
    Fuzzy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexingState {
    pub status: IndexingStatus,
    pub has_initial_index: bool,
    pub indexed_item_count: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IndexingStatus {
    NotStarted,
    InitialBuild,
    Reconciling,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationActivity {
    pub id: String,
    pub operation: FileOperationKind,
    pub status: OperationStatus,
    pub primary_path: Option<String>,
    pub message: String,
    pub progress: Option<OperationProgress>,
    pub recoverable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileOperationKind {
    NewFolder,
    Rename,
    MoveToTrash,
    Copy,
    Move,
    Open,
    OpenWith,
    Indexing,
    Preview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OperationStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgress {
    pub completed: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDescriptor {
    pub path: String,
    pub renderer: PreviewRenderer,
    pub display_name: String,
    pub kind: String,
    pub size: Option<u64>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PreviewRenderer {
    Image,
    Video,
    Audio,
    Text,
    Pdf,
    FallbackMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapabilities {
    pub native_titlebar_tabs: bool,
    pub open_with_chooser: bool,
    pub reliable_trash_undo: bool,
    pub outbound_file_drag: bool,
    pub cloud_placeholder_detection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EventNames {
    pub directory_listing_progress: String,
    pub indexing_progress: String,
    pub file_operation_progress: String,
    pub watcher_update: String,
    pub settings_changed: String,
    pub activity_failure: String,
}

impl Default for EventNames {
    fn default() -> Self {
        Self {
            directory_listing_progress: "frogger://directory-listing-progress".to_string(),
            indexing_progress: "frogger://indexing-progress".to_string(),
            file_operation_progress: "frogger://file-operation-progress".to_string(),
            watcher_update: "frogger://watcher-update".to_string(),
            settings_changed: "frogger://settings-changed".to_string(),
            activity_failure: "frogger://activity-failure".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn app_bootstrap_serializes_with_camel_case_fields() {
        let bootstrap = AppBootstrap {
            app_version: "0.1.0".to_string(),
            platform: PlatformInfo {
                os: "macos".to_string(),
                family: "unix".to_string(),
                path_separator: "/".to_string(),
                home_dir: Some("/Users/example".to_string()),
            },
            access: FileAccessState {
                status: FileAccessStatus::Granted,
                home_dir: Some("/Users/example".to_string()),
                message: None,
                recovery_hint: None,
            },
            settings: AppSettings {
                appearance_mode: AppearanceMode::System,
                hidden_files_visible: false,
                file_extensions_visible: false,
                folders_first: true,
                path_bar_visible: true,
                restore_enabled: true,
                local_only_indexing: true,
                previews_enabled: true,
                list_column_visibility: BTreeMap::new(),
                list_column_widths: BTreeMap::new(),
                raw: BTreeMap::new(),
            },
            windows: vec![],
            sidebar: SidebarState {
                sections: vec![],
                recent_items: vec![],
                favorites: vec![],
                locations: vec![],
                recents_virtual_folder_id: "recents".to_string(),
            },
            indexing: IndexingState {
                status: IndexingStatus::NotStarted,
                has_initial_index: false,
                indexed_item_count: 0,
                message: None,
            },
            capabilities: AppCapabilities {
                native_titlebar_tabs: true,
                open_with_chooser: true,
                reliable_trash_undo: false,
                outbound_file_drag: true,
                cloud_placeholder_detection: true,
            },
            events: EventNames::default(),
        };

        let value = serde_json::to_value(bootstrap).expect("bootstrap should serialize");
        assert_eq!(value["appVersion"], json!("0.1.0"));
        assert_eq!(value["access"]["status"], json!("granted"));
        assert_eq!(value["settings"]["foldersFirst"], json!(true));
        assert_eq!(
            value["events"]["indexingProgress"],
            json!("frogger://indexing-progress")
        );
    }

    #[test]
    fn file_entry_serializes_visible_metadata_contract() {
        let entry = FileEntry {
            path: "/Users/example/image.png".to_string(),
            parent_path: "/Users/example".to_string(),
            name: "image.png".to_string(),
            display_name: "image".to_string(),
            kind: "PNG Image".to_string(),
            is_dir: false,
            size: Some(1024),
            modified_at: Some("2026-04-28T00:00:00Z".to_string()),
            created_at: None,
            hidden: false,
            extension: Some("png".to_string()),
            read_only: false,
            icon: FileIcon {
                name: "image".to_string(),
                color: Some("blue".to_string()),
            },
            cloud: CloudState::Local,
            is_symlink: false,
            symlink_broken: false,
            symlink_target: None,
        };

        let value = serde_json::to_value(entry).expect("entry should serialize");
        assert_eq!(value["displayName"], json!("image"));
        assert_eq!(value["isDir"], json!(false));
        assert_eq!(value["cloud"], json!("local"));
    }
}
