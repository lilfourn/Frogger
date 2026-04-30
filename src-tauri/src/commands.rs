use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use image::ImageReader;
use rusqlite::{params, Connection};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

use crate::errors::CommandError;
use crate::models::{
    AppBootstrap, AppCapabilities, AppSettings, AppearanceMode, CloudState, DirectoryListRequest,
    DirectoryListing, EventNames, FileAccessState, FileAccessStatus, FileEntry, FileIcon,
    FolderViewState, IndexingState, IndexingStatus, PlatformInfo, SearchMatchReason, SearchResult,
    SidebarItem, SidebarItemType, SidebarSectionId, SidebarSectionState, SidebarState,
    SortDirection, SortKey, SortState, TabState, ThumbnailDescriptor, ViewMode, WindowGeometry,
    WindowState,
};
use crate::persistence;

const RECENTS_VIRTUAL_PATH: &str = "recents";

pub(crate) fn restored_windows_for_app(app: &tauri::AppHandle) -> Result<Vec<WindowState>> {
    let database_path = app
        .path()
        .app_data_dir()
        .context("failed to resolve app data directory")?
        .join("frogger.sqlite3");
    let conn = persistence::open_database(&database_path)?;
    let settings = load_settings(&conn)?;
    let Some(home_dir) = home_dir_string() else {
        return Ok(Vec::new());
    };

    if !matches!(
        detect_file_access(Some(&home_dir)).status,
        FileAccessStatus::Granted
    ) {
        return Ok(Vec::new());
    }

    restore_windows(&conn, &home_dir, &settings)
}

#[tauri::command]
pub fn bootstrap_app(app: tauri::AppHandle) -> Result<AppBootstrap, CommandError> {
    let database_path = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::internal(error.to_string()))?
        .join("frogger.sqlite3");
    let conn = persistence::open_database(&database_path).map_err(CommandError::from)?;
    let settings = load_settings(&conn).map_err(CommandError::from)?;
    let home_dir = home_dir_string();
    let access = detect_file_access(home_dir.as_deref());
    let windows = match &access.status {
        FileAccessStatus::Granted => home_dir
            .as_deref()
            .map(|path| restore_windows(&conn, path, &settings))
            .transpose()
            .map_err(CommandError::from)?
            .unwrap_or_default(),
        FileAccessStatus::Denied => Vec::new(),
    };

    if matches!(&access.status, FileAccessStatus::Granted) {
        if let Some(home) = home_dir.as_deref() {
            if let Err(error) = crate::indexing::ensure_metadata_index_started(
                &app,
                database_path.clone(),
                PathBuf::from(home),
            ) {
                #[cfg(debug_assertions)]
                eprintln!("[frogger] failed to start metadata indexer: {error}");
            }
        }
    }

    Ok(AppBootstrap {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: PlatformInfo {
            os: std::env::consts::OS.to_string(),
            family: std::env::consts::FAMILY.to_string(),
            path_separator: std::path::MAIN_SEPARATOR.to_string(),
            home_dir: home_dir.clone(),
        },
        access,
        settings,
        windows,
        sidebar: load_sidebar_state(&conn, home_dir).map_err(CommandError::from)?,
        indexing: load_indexing_state(&conn).map_err(CommandError::from)?,
        capabilities: build_capabilities(),
        events: EventNames::default(),
    })
}

#[tauri::command]
pub fn save_session_state(
    app: tauri::AppHandle,
    windows: Vec<WindowState>,
) -> Result<(), CommandError> {
    let database_path = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::internal(error.to_string()))?
        .join("frogger.sqlite3");
    let mut conn = persistence::open_database(&database_path).map_err(CommandError::from)?;
    save_windows(&mut conn, &windows).map_err(CommandError::from)
}

#[tauri::command]
pub fn create_file_manager_window(
    app: tauri::AppHandle,
    path: Option<String>,
) -> Result<WindowState, CommandError> {
    let database_path = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::internal(error.to_string()))?
        .join("frogger.sqlite3");
    let mut conn = persistence::open_database(&database_path).map_err(CommandError::from)?;
    let settings = load_settings(&conn).map_err(CommandError::from)?;
    let home_dir = home_dir_string().ok_or_else(|| {
        CommandError::unavailable(
            "No home directory was detected for this user.",
            Some(
                "Check the operating system account and filesystem permissions, then retry."
                    .to_string(),
            ),
        )
    })?;
    let target_path = path.unwrap_or_else(|| home_dir.clone());

    if !Path::new(&target_path).is_dir() || Path::new(&target_path).read_dir().is_err() {
        return Err(CommandError::unavailable(
            "The requested folder is unavailable.",
            Some(target_path),
        ));
    }

    let window = window_for_path(
        format!("window-{}", Uuid::new_v4()),
        format!("window-{}", Uuid::new_v4()),
        format!("tab-{}", Uuid::new_v4()),
        &target_path,
        folder_title(&target_path),
        &settings,
    );

    let previous_windows =
        restore_windows(&conn, &home_dir, &settings).map_err(CommandError::from)?;
    let mut persisted_windows = previous_windows.clone();
    persisted_windows.retain(|existing| existing.id != window.id && existing.label != window.label);
    persisted_windows.push(window.clone());
    save_windows(&mut conn, &persisted_windows).map_err(CommandError::from)?;

    let build_result = WebviewWindowBuilder::new(
        &app,
        window.label.clone(),
        WebviewUrl::App("index.html".into()),
    )
    .title("Frogger")
    .title_bar_style(tauri::TitleBarStyle::Overlay)
    .hidden_title(true)
    .inner_size(window.geometry.width, window.geometry.height)
    .center()
    .build();

    if let Err(error) = build_result {
        let _ = save_windows(&mut conn, &previous_windows);
        return Err(CommandError::internal(error.to_string()));
    }

    Ok(window)
}

#[tauri::command]
pub fn list_directory(
    app: tauri::AppHandle,
    request: DirectoryListRequest,
) -> Result<DirectoryListing, CommandError> {
    if is_recents_virtual_path(&request.path) {
        let conn = open_app_database(&app)?;
        return list_recents_directory_impl(
            &conn,
            request.hidden_files_visible,
            request.file_extensions_visible,
            request.cursor.as_deref(),
            request.limit,
        );
    }

    list_directory_impl(
        request.path,
        &request.sort,
        request.folders_first,
        request.hidden_files_visible,
        request.file_extensions_visible,
        request.cursor.as_deref(),
        request.limit,
    )
}

#[tauri::command]
pub fn search_metadata(
    app: tauri::AppHandle,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, CommandError> {
    let conn = open_app_database(&app)?;
    search_metadata_impl(&conn, &query, limit).map_err(CommandError::from)
}

#[tauri::command]
pub fn open_file_with_default_app(
    app: tauri::AppHandle,
    path: String,
) -> Result<SidebarState, CommandError> {
    let target = Path::new(&path);
    let metadata = std::fs::metadata(target).map_err(|error| fs_access_error(target, error))?;
    if metadata.is_dir() {
        return Err(CommandError::unavailable(
            "Folders are opened inside Frogger, not through the default app command.",
            Some(path),
        ));
    }

    tauri_plugin_opener::open_path(target, None::<&str>).map_err(|error| {
        CommandError::unavailable("The file could not be opened.", Some(error.to_string()))
    })?;

    let conn = open_app_database(&app)?;
    record_recent_path(&conn, &path)?;
    load_sidebar_state(&conn, home_dir_string()).map_err(CommandError::from)
}

#[tauri::command]
pub fn get_sidebar_state(app: tauri::AppHandle) -> Result<SidebarState, CommandError> {
    let conn = open_app_database(&app)?;
    load_sidebar_state(&conn, home_dir_string()).map_err(CommandError::from)
}

#[tauri::command]
pub fn pin_sidebar_folder(
    app: tauri::AppHandle,
    path: String,
    label: Option<String>,
) -> Result<SidebarState, CommandError> {
    if !Path::new(&path).is_dir() {
        return Err(CommandError::unavailable(
            "Only folders can be pinned to Favorites.",
            Some(path),
        ));
    }

    let conn = open_app_database(&app)?;
    let position = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM favorites",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(anyhow::Error::from)
        .map_err(CommandError::from)?;
    let favorite_label = label
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| folder_title(&path));

    conn.execute(
        "INSERT INTO favorites (id, path, label, position)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(path) DO UPDATE SET
            label = excluded.label,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        params![
            format!("favorite-{}", Uuid::new_v4()),
            path,
            favorite_label,
            position
        ],
    )
    .map_err(anyhow::Error::from)
    .map_err(CommandError::from)?;

    load_sidebar_state(&conn, home_dir_string()).map_err(CommandError::from)
}

#[tauri::command]
pub fn unpin_sidebar_folder(
    app: tauri::AppHandle,
    path: String,
) -> Result<SidebarState, CommandError> {
    let conn = open_app_database(&app)?;
    conn.execute("DELETE FROM favorites WHERE path = ?1", [path])
        .map_err(anyhow::Error::from)
        .map_err(CommandError::from)?;
    load_sidebar_state(&conn, home_dir_string()).map_err(CommandError::from)
}

#[tauri::command]
pub fn set_sidebar_section_visibility(
    app: tauri::AppHandle,
    section_id: String,
    visible: bool,
) -> Result<SidebarState, CommandError> {
    let section = sidebar_section_from_db(&section_id).ok_or_else(|| {
        CommandError::unavailable("Unknown sidebar section.", Some(section_id.clone()))
    })?;
    let conn = open_app_database(&app)?;
    conn.execute(
        "UPDATE sidebar_sections SET visible = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2",
        params![bool_to_i64(visible), sidebar_section_to_db(&section)],
    )
    .map_err(anyhow::Error::from)
    .map_err(CommandError::from)?;
    load_sidebar_state(&conn, home_dir_string()).map_err(CommandError::from)
}

#[tauri::command]
pub fn cleanup_thumbnail_cache(app: tauri::AppHandle) -> Result<usize, CommandError> {
    let conn = open_app_database(&app)?;
    cleanup_stale_thumbnail_metadata(&conn)
}

#[tauri::command]
pub fn get_thumbnail(
    app: tauri::AppHandle,
    path: String,
) -> Result<Option<ThumbnailDescriptor>, CommandError> {
    let target = PathBuf::from(&path);
    if !is_supported_thumbnail_source(&target) {
        return Ok(None);
    }

    let metadata = std::fs::metadata(&target).map_err(|error| fs_access_error(&target, error))?;
    if metadata.is_dir() || metadata.len() == 0 {
        return Ok(None);
    }

    let modified_at = metadata.modified().ok().map(system_time_to_rfc3339);
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| CommandError::internal(error.to_string()))?
        .join("thumbnails");
    let conn = open_app_database(&app)?;

    get_or_generate_thumbnail(
        &conn,
        &cache_dir,
        &target,
        metadata.len(),
        modified_at.as_deref(),
    )
}

#[tauri::command]
pub fn record_recent_item(
    app: tauri::AppHandle,
    path: String,
) -> Result<SidebarState, CommandError> {
    if !Path::new(&path).exists() {
        return Err(CommandError::unavailable(
            "Only existing items can be added to Recents.",
            Some(path),
        ));
    }

    let conn = open_app_database(&app)?;
    record_recent_path(&conn, &path)?;
    load_sidebar_state(&conn, home_dir_string()).map_err(CommandError::from)
}

#[tauri::command]
pub fn set_browser_display_setting(
    app: tauri::AppHandle,
    key: String,
    value: String,
) -> Result<AppSettings, CommandError> {
    if !is_allowed_display_setting(&key, &value) {
        return Err(CommandError::unavailable(
            "Unsupported display setting.",
            Some(format!("{key}={value}")),
        ));
    }

    let conn = open_app_database(&app)?;
    conn.execute(
        "INSERT INTO settings (key, value, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at",
        params![key, value],
    )
    .map_err(anyhow::Error::from)
    .map_err(CommandError::from)?;
    load_settings(&conn).map_err(CommandError::from)
}

#[tauri::command]
pub fn get_folder_view_state(
    app: tauri::AppHandle,
    path: String,
) -> Result<FolderViewState, CommandError> {
    let conn = open_app_database(&app)?;
    let settings = load_settings(&conn).map_err(CommandError::from)?;
    load_folder_view_state(&conn, &path, &settings).map_err(CommandError::from)
}

#[tauri::command]
pub fn save_folder_view_state(
    app: tauri::AppHandle,
    path: String,
    state: FolderViewState,
) -> Result<(), CommandError> {
    let conn = open_app_database(&app)?;
    save_folder_view_state_impl(&conn, &path, &state).map_err(CommandError::from)
}

fn list_recents_directory_impl(
    conn: &Connection,
    hidden_files_visible: bool,
    file_extensions_visible: bool,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<DirectoryListing, CommandError> {
    let mut stmt = conn
        .prepare("SELECT path FROM recents ORDER BY opened_at DESC, open_count DESC")
        .map_err(anyhow::Error::from)
        .map_err(CommandError::from)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(anyhow::Error::from)
        .map_err(CommandError::from)?;

    let mut entries = Vec::new();
    for row in rows {
        let path = row
            .map_err(anyhow::Error::from)
            .map_err(CommandError::from)?;
        let path = PathBuf::from(path);
        if let Ok(Some(entry)) = file_entry_from_path(
            path.parent().unwrap_or_else(|| Path::new("")),
            &path,
            hidden_files_visible,
            file_extensions_visible,
        ) {
            entries.push(entry);
        }
    }

    let total_count = entries.len();
    let offset = cursor
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(total_count);
    let requested_limit = limit.unwrap_or(total_count.saturating_sub(offset));
    let next_offset = offset.saturating_add(requested_limit).min(total_count);
    let page = entries
        .into_iter()
        .skip(offset)
        .take(requested_limit)
        .collect::<Vec<_>>();

    Ok(DirectoryListing {
        path: RECENTS_VIRTUAL_PATH.to_string(),
        entries: page,
        total_count,
        next_cursor: (next_offset < total_count).then(|| next_offset.to_string()),
        loading_complete: next_offset >= total_count,
    })
}

fn list_directory_impl(
    path: String,
    sort: &SortState,
    folders_first: bool,
    hidden_files_visible: bool,
    file_extensions_visible: bool,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<DirectoryListing, CommandError> {
    let target = PathBuf::from(&path);
    // Diagnostic (dev-only): log the incoming path bytes and existence so we
    // can tell the difference between a path-mangling bug (wrong string) and a
    // real OS-level failure (EACCES/EPERM/ENOENT) when the UI reports
    // "Folder unavailable". Compiled out of release builds.
    #[cfg(debug_assertions)]
    eprintln!(
        "[frogger] list_directory target={target:?} raw_len={raw_len} exists={exists} is_dir={is_dir} is_symlink={is_symlink}",
        target = target,
        raw_len = path.len(),
        exists = target.exists(),
        is_dir = target.is_dir(),
        is_symlink = target.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false),
    );
    let metadata = std::fs::metadata(&target).map_err(|error| {
        #[cfg(debug_assertions)]
        eprintln!(
            "[frogger] list_directory metadata() failed target={target:?} kind={kind:?} raw_os_error={errno:?} error={error}",
            target = target,
            kind = error.kind(),
            errno = error.raw_os_error(),
            error = error,
        );
        fs_access_error(&target, error)
    })?;
    if !metadata.is_dir() {
        return Err(CommandError::unavailable(
            "The requested path is not a folder.",
            Some(path),
        ));
    }

    let mut entries = Vec::new();
    let directory = std::fs::read_dir(&target).map_err(|error| {
        #[cfg(debug_assertions)]
        eprintln!(
            "[frogger] list_directory read_dir() failed target={target:?} kind={kind:?} raw_os_error={errno:?} error={error}",
            target = target,
            kind = error.kind(),
            errno = error.raw_os_error(),
            error = error,
        );
        fs_access_error(&target, error)
    })?;
    for dir_entry in directory {
        let dir_entry = match dir_entry {
            Ok(entry) => entry,
            Err(_error) => {
                // One unreadable entry must not abort the whole listing
                // (e.g. a race where the file vanishes mid-iteration).
                #[cfg(debug_assertions)]
                eprintln!(
                    "[frogger] list_directory skipping unreadable entry in {target:?}: {_error}",
                );
                continue;
            }
        };
        match file_entry_from_dir_entry(
            &target,
            dir_entry,
            hidden_files_visible,
            file_extensions_visible,
        ) {
            Ok(Some(entry)) => entries.push(entry),
            Ok(None) => {}
            Err(_error) => {
                // Per-entry failure (broken symlink, permission on a single
                // child, ...) must not fail the whole listing — otherwise
                // something like a dangling ~/python3 symlink makes $HOME
                // completely unbrowsable.
                #[cfg(debug_assertions)]
                eprintln!("[frogger] list_directory skipping entry in {target:?}: {_error:?}",);
            }
        }
    }

    sort_entries(&mut entries, sort, folders_first);

    let total_count = entries.len();
    let offset = cursor
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(total_count);

    let requested_limit = limit.unwrap_or(total_count.saturating_sub(offset));
    let next_offset = offset.saturating_add(requested_limit).min(total_count);
    let page = entries
        .into_iter()
        .skip(offset)
        .take(requested_limit)
        .collect::<Vec<_>>();

    Ok(DirectoryListing {
        path: target.to_string_lossy().into_owned(),
        entries: page,
        total_count,
        next_cursor: (next_offset < total_count).then(|| next_offset.to_string()),
        loading_complete: next_offset >= total_count,
    })
}

fn file_entry_from_dir_entry(
    parent: &Path,
    dir_entry: std::fs::DirEntry,
    hidden_files_visible: bool,
    file_extensions_visible: bool,
) -> Result<Option<FileEntry>, CommandError> {
    file_entry_from_path(
        parent,
        &dir_entry.path(),
        hidden_files_visible,
        file_extensions_visible,
    )
}

fn file_entry_from_path(
    parent: &Path,
    path: &Path,
    hidden_files_visible: bool,
    file_extensions_visible: bool,
) -> Result<Option<FileEntry>, CommandError> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let hidden = is_hidden_name(&name);
    if hidden && !hidden_files_visible {
        return Ok(None);
    }

    // Use symlink_metadata() first so we can detect the entry even when it is
    // a broken symlink (follow-through metadata() would return ENOENT and
    // previously caused the whole parent directory to be reported as missing).
    let link_metadata =
        std::fs::symlink_metadata(path).map_err(|error| fs_access_error(path, error))?;
    let is_symlink = link_metadata.file_type().is_symlink();

    // For symlinks, resolve the target so we can report the correct is_dir and
    // size. If the target is missing (broken symlink), fall back to the link's
    // own metadata and treat the entry as a non-directory so the UI does not
    // try to descend into it.
    let (metadata, target_resolved) = if is_symlink {
        match std::fs::metadata(path) {
            Ok(target_meta) => (target_meta, true),
            Err(_error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[frogger] broken symlink {path:?} -> target unreachable ({kind:?}); including as non-directory entry",
                    path = path,
                    kind = _error.kind(),
                );
                (link_metadata.clone(), false)
            }
        }
    } else {
        (link_metadata.clone(), true)
    };
    let symlink_target = if is_symlink {
        std::fs::read_link(path)
            .ok()
            .map(|target| target.to_string_lossy().into_owned())
    } else {
        None
    };
    let symlink_broken = is_symlink && !target_resolved;
    let is_dir = metadata.is_dir() && target_resolved;
    let extension = (!is_dir)
        .then(|| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .flatten();
    let display_name = display_name_for(path, &name, is_dir, file_extensions_visible);
    let kind = if symlink_broken {
        "Alias (broken)"
    } else if is_symlink && is_dir {
        "Alias (folder)"
    } else if is_symlink {
        "Alias"
    } else {
        kind_for(is_dir, extension.as_deref())
    };

    let icon = icon_for(is_dir, extension.as_deref());

    Ok(Some(FileEntry {
        path: path.to_string_lossy().into_owned(),
        parent_path: parent.to_string_lossy().into_owned(),
        name,
        display_name,
        kind: kind.to_string(),
        is_dir,
        size: (!is_dir).then_some(metadata.len()),
        modified_at: metadata.modified().ok().map(system_time_to_rfc3339),
        created_at: metadata.created().ok().map(system_time_to_rfc3339),
        hidden,
        extension,
        read_only: metadata.permissions().readonly(),
        icon,
        cloud: CloudState::Local,
        is_symlink,
        symlink_broken,
        symlink_target,
    }))
}

fn display_name_for(
    path: &Path,
    name: &str,
    is_dir: bool,
    file_extensions_visible: bool,
) -> String {
    if is_dir || file_extensions_visible {
        return name.to_string();
    }

    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| name.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileCategory {
    Application,
    Archive,
    Audio,
    Document,
    Image,
    Markdown,
    Pdf,
    SourceCode,
    Spreadsheet,
    Text,
    Video,
    WordDocument,
}

fn file_category(extension: Option<&str>) -> FileCategory {
    match extension.map(|value| value.to_ascii_lowercase()).as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "svg") => FileCategory::Image,
        Some("mov" | "mp4" | "m4v" | "avi" | "mkv" | "webm") => FileCategory::Video,
        Some("mp3" | "wav" | "aac" | "flac" | "m4a" | "ogg") => FileCategory::Audio,
        Some("pdf") => FileCategory::Pdf,
        Some("xls" | "xlsx" | "xlsm" | "xlsb" | "csv" | "tsv" | "ods" | "numbers") => {
            FileCategory::Spreadsheet
        }
        Some("doc" | "docx" | "odt" | "pages") => FileCategory::WordDocument,
        Some("md" | "markdown") => FileCategory::Markdown,
        Some("txt" | "rtf" | "log") => FileCategory::Text,
        Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar") => FileCategory::Archive,
        Some("app" | "exe" | "dmg" | "pkg") => FileCategory::Application,
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "json" | "html" | "css" | "scss") => {
            FileCategory::SourceCode
        }
        Some(_) | None => FileCategory::Document,
    }
}

fn kind_for(is_dir: bool, extension: Option<&str>) -> &'static str {
    if is_dir {
        return "Folder";
    }

    match file_category(extension) {
        FileCategory::Application => "Application",
        FileCategory::Archive => "Archive",
        FileCategory::Audio => "Audio",
        FileCategory::Document => "Document",
        FileCategory::Image => "Image",
        FileCategory::Markdown => "Markdown Document",
        FileCategory::Pdf => "PDF Document",
        FileCategory::SourceCode => "Source Code",
        FileCategory::Spreadsheet => "Spreadsheet",
        FileCategory::Text => "Text Document",
        FileCategory::Video => "Video",
        FileCategory::WordDocument => "Word Document",
    }
}

fn icon_for(is_dir: bool, extension: Option<&str>) -> FileIcon {
    if is_dir {
        return FileIcon {
            name: "folder".to_string(),
            color: Some("blue".to_string()),
        };
    }

    let name = match file_category(extension) {
        FileCategory::Archive => "archive",
        FileCategory::Document => "generic",
        FileCategory::Markdown => "markdown",
        FileCategory::Pdf => "pdf",
        FileCategory::Spreadsheet => "spreadsheet",
        FileCategory::WordDocument => "word-document",
        FileCategory::Application
        | FileCategory::Audio
        | FileCategory::Image
        | FileCategory::SourceCode
        | FileCategory::Text
        | FileCategory::Video => "file",
    };

    FileIcon {
        name: name.to_string(),
        color: None,
    }
}

#[derive(Debug)]
struct SearchCandidate {
    result: SearchResult,
    search_text: String,
    recent_boost: f64,
    modified_boost: f64,
}

fn search_metadata_impl(
    conn: &Connection,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>> {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() {
        return Ok(Vec::new());
    }

    let result_limit = limit.unwrap_or(50).clamp(1, 200);
    let candidate_limit = (result_limit * 25).clamp(250, 5_000);
    let escaped_query = escape_sql_like(&normalized_query);
    let prefix_pattern = format!("{escaped_query}%");
    let contains_pattern = format!("%{escaped_query}%");
    let first_char_pattern = normalized_query
        .chars()
        .next()
        .map(|character| format!("%{}%", escape_sql_like(&character.to_string())))
        .unwrap_or_else(|| contains_pattern.clone());

    let mut stmt = conn.prepare(
        "SELECT path, parent_path, name, display_name, kind, is_dir, size, modified_at,
                search_text, recent_boost, modified_boost
         FROM metadata_index
         WHERE lower(name) = ?1
            OR lower(display_name) = ?1
            OR lower(name) LIKE ?2 ESCAPE '\\'
            OR search_text LIKE ?3 ESCAPE '\\'
            OR lower(path) LIKE ?3 ESCAPE '\\'
            OR lower(name) LIKE ?4 ESCAPE '\\'
         ORDER BY is_dir DESC, recent_boost DESC, modified_at DESC, name ASC
         LIMIT ?5",
    )?;

    let rows = stmt.query_map(
        params![
            &normalized_query,
            &prefix_pattern,
            &contains_pattern,
            &first_char_pattern,
            candidate_limit as i64,
        ],
        |row| {
            let size = row
                .get::<_, Option<i64>>(6)?
                .and_then(|value| (value >= 0).then_some(value as u64));
            Ok(SearchCandidate {
                result: SearchResult {
                    path: row.get(0)?,
                    parent_path: row.get(1)?,
                    name: row.get(2)?,
                    display_name: row.get(3)?,
                    kind: row.get(4)?,
                    is_dir: row.get::<_, i64>(5)? == 1,
                    size,
                    modified_at: row.get(7)?,
                    rank: 0,
                    match_reason: SearchMatchReason::Substring,
                },
                search_text: row.get(8)?,
                recent_boost: row.get(9)?,
                modified_boost: row.get(10)?,
            })
        },
    )?;

    let matcher = SkimMatcherV2::default();
    let mut ranked = Vec::new();
    for row in rows {
        let candidate = row?;
        if let Some((rank, reason)) =
            score_search_candidate(&candidate, &normalized_query, &matcher)
        {
            let mut result = candidate.result;
            result.rank = rank;
            result.match_reason = reason;
            ranked.push(result);
        }
    }

    ranked.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| right.is_dir.cmp(&left.is_dir))
            .then_with(|| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            })
            .then_with(|| left.path.cmp(&right.path))
    });
    ranked.truncate(result_limit);
    Ok(ranked)
}

fn score_search_candidate(
    candidate: &SearchCandidate,
    query: &str,
    matcher: &SkimMatcherV2,
) -> Option<(i64, SearchMatchReason)> {
    let name = candidate.result.name.to_ascii_lowercase();
    let display_name = candidate.result.display_name.to_ascii_lowercase();
    let path = candidate.result.path.to_ascii_lowercase();
    let search_text = candidate.search_text.to_ascii_lowercase();

    let (base, reason, fuzzy_score) = if name == query || display_name == query {
        (1_000_000_i64, SearchMatchReason::Exact, 0_i64)
    } else if name.starts_with(query) || display_name.starts_with(query) {
        (800_000_i64, SearchMatchReason::Prefix, 0_i64)
    } else if search_text.contains(query) || path.contains(query) {
        (600_000_i64, SearchMatchReason::Substring, 0_i64)
    } else {
        let fuzzy_score = [
            matcher.fuzzy_match(&candidate.result.name, query),
            matcher.fuzzy_match(&candidate.result.display_name, query),
            matcher.fuzzy_match(&candidate.result.parent_path, query),
            matcher.fuzzy_match(&candidate.search_text, query),
        ]
        .into_iter()
        .flatten()
        .max()?;
        (400_000_i64, SearchMatchReason::Fuzzy, fuzzy_score)
    };

    let folder_boost = if candidate.result.is_dir { 100_000 } else { 0 };
    let recent_boost = (candidate.recent_boost * 1_000.0).round() as i64;
    let modified_boost = (candidate.modified_boost * 1_000.0).round() as i64;
    Some((
        base + folder_boost + fuzzy_score + recent_boost + modified_boost,
        reason,
    ))
}

fn escape_sql_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn sort_entries(entries: &mut [FileEntry], sort: &SortState, folders_first: bool) {
    entries.sort_by(|left, right| {
        if folders_first && left.is_dir != right.is_dir {
            return right.is_dir.cmp(&left.is_dir);
        }

        let primary = compare_entries(left, right, &sort.key);
        let directed = match sort.direction {
            SortDirection::Asc => primary,
            SortDirection::Desc => primary.reverse(),
        };

        directed.then_with(|| compare_entries(left, right, &SortKey::Name))
    });
}

fn compare_entries(left: &FileEntry, right: &FileEntry, key: &SortKey) -> Ordering {
    match key {
        SortKey::Name => left
            .display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase()),
        SortKey::DateModified => left.modified_at.cmp(&right.modified_at),
        SortKey::Size => left.size.unwrap_or(0).cmp(&right.size.unwrap_or(0)),
        SortKey::Kind => left.kind.cmp(&right.kind),
        SortKey::Path => left.path.cmp(&right.path),
    }
}

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

fn is_supported_thumbnail_source(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };

    if path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|name| name.ends_with(".icloud"))
        .unwrap_or(false)
    {
        return false;
    }

    matches!(
        extension.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "webp"
    )
}

fn thumbnail_cache_name(
    source_path: &Path,
    source_size: u64,
    source_modified_at: Option<&str>,
) -> String {
    let mut hasher = DefaultHasher::new();
    source_path.to_string_lossy().hash(&mut hasher);
    source_size.hash(&mut hasher);
    source_modified_at.hash(&mut hasher);
    format!("{:016x}.png", hasher.finish())
}

fn get_or_generate_thumbnail(
    conn: &Connection,
    cache_dir: &Path,
    source_path: &Path,
    source_size: u64,
    source_modified_at: Option<&str>,
) -> Result<Option<ThumbnailDescriptor>, CommandError> {
    let cached = lookup_valid_thumbnail(conn, source_path, source_size, source_modified_at)?;
    if let Some(descriptor) = cached {
        return Ok(Some(descriptor));
    }

    std::fs::create_dir_all(cache_dir).map_err(|error| {
        CommandError::internal(format!(
            "failed to create thumbnail cache directory: {error}"
        ))
    })?;

    let thumbnail_path = cache_dir.join(thumbnail_cache_name(
        source_path,
        source_size,
        source_modified_at,
    ));
    let image = ImageReader::open(source_path)
        .map_err(|error| {
            CommandError::unavailable(
                "The image could not be opened for thumbnailing.",
                Some(error.to_string()),
            )
        })?
        .decode()
        .map_err(|error| {
            CommandError::unavailable(
                "The image could not be decoded for thumbnailing.",
                Some(error.to_string()),
            )
        })?;
    let thumbnail = image.thumbnail(320, 320);
    thumbnail.save(&thumbnail_path).map_err(|error| {
        CommandError::internal(format!("thumbnail could not be written: {error}"))
    })?;

    let width = thumbnail.width();
    let height = thumbnail.height();
    upsert_thumbnail_metadata(
        conn,
        source_path,
        &thumbnail_path,
        source_size,
        source_modified_at,
        width,
        height,
    )?;

    Ok(Some(ThumbnailDescriptor {
        source_path: source_path.to_string_lossy().into_owned(),
        thumbnail_path: thumbnail_path.to_string_lossy().into_owned(),
        width,
        height,
        source_modified_at: source_modified_at.map(ToString::to_string),
        source_size,
        cache_hit: false,
    }))
}

fn lookup_valid_thumbnail(
    conn: &Connection,
    source_path: &Path,
    source_size: u64,
    source_modified_at: Option<&str>,
) -> Result<Option<ThumbnailDescriptor>, CommandError> {
    let source = source_path.to_string_lossy().into_owned();
    let mut stmt = conn
        .prepare(
            "SELECT thumbnail_path, source_modified_at, source_size, width, height
             FROM thumbnail_metadata WHERE source_path = ?1",
        )
        .map_err(anyhow::Error::from)
        .map_err(CommandError::from)?;
    let result = stmt.query_row([source.as_str()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<u64>>(2)?,
            row.get::<_, u32>(3)?,
            row.get::<_, u32>(4)?,
        ))
    });

    match result {
        Ok((thumbnail_path, cached_modified_at, cached_size, width, height))
            if cached_modified_at.as_deref() == source_modified_at
                && cached_size == Some(source_size)
                && Path::new(&thumbnail_path).is_file() =>
        {
            conn.execute(
                "UPDATE thumbnail_metadata SET last_accessed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE source_path = ?1",
                [source.as_str()],
            )
            .map_err(anyhow::Error::from)
            .map_err(CommandError::from)?;
            Ok(Some(ThumbnailDescriptor {
                source_path: source,
                thumbnail_path,
                width,
                height,
                source_modified_at: source_modified_at.map(ToString::to_string),
                source_size,
                cache_hit: true,
            }))
        }
        Ok((thumbnail_path, _, _, _, _)) => {
            std::fs::remove_file(thumbnail_path).ok();
            conn.execute(
                "DELETE FROM thumbnail_metadata WHERE source_path = ?1",
                [source],
            )
            .map_err(anyhow::Error::from)
            .map_err(CommandError::from)?;
            Ok(None)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(CommandError::from(anyhow::Error::from(error))),
    }
}

fn upsert_thumbnail_metadata(
    conn: &Connection,
    source_path: &Path,
    thumbnail_path: &Path,
    source_size: u64,
    source_modified_at: Option<&str>,
    width: u32,
    height: u32,
) -> Result<(), CommandError> {
    conn.execute(
        "INSERT INTO thumbnail_metadata (
            source_path, thumbnail_path, source_modified_at, source_size, width, height,
            generated_at, last_accessed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(source_path) DO UPDATE SET
            thumbnail_path = excluded.thumbnail_path,
            source_modified_at = excluded.source_modified_at,
            source_size = excluded.source_size,
            width = excluded.width,
            height = excluded.height,
            generated_at = excluded.generated_at,
            last_accessed_at = excluded.last_accessed_at",
        params![
            source_path.to_string_lossy().as_ref(),
            thumbnail_path.to_string_lossy().as_ref(),
            source_modified_at,
            source_size,
            width,
            height,
        ],
    )
    .map(|_| ())
    .map_err(anyhow::Error::from)
    .map_err(CommandError::from)
}

fn cleanup_stale_thumbnail_metadata(conn: &Connection) -> Result<usize, CommandError> {
    let mut stmt = conn
        .prepare("SELECT source_path, thumbnail_path FROM thumbnail_metadata")
        .map_err(anyhow::Error::from)
        .map_err(CommandError::from)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(anyhow::Error::from)
        .map_err(CommandError::from)?;

    let mut stale_sources = Vec::new();
    for row in rows {
        let (source_path, thumbnail_path) = row
            .map_err(anyhow::Error::from)
            .map_err(CommandError::from)?;
        if !Path::new(&source_path).exists() || !Path::new(&thumbnail_path).exists() {
            std::fs::remove_file(thumbnail_path).ok();
            stale_sources.push(source_path);
        }
    }

    for source in &stale_sources {
        conn.execute(
            "DELETE FROM thumbnail_metadata WHERE source_path = ?1",
            [source],
        )
        .map_err(anyhow::Error::from)
        .map_err(CommandError::from)?;
    }

    Ok(stale_sources.len())
}

fn is_recents_virtual_path(path: &str) -> bool {
    path == RECENTS_VIRTUAL_PATH || path == "frogger://recents"
}

fn system_time_to_rfc3339(value: SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339()
}

fn fs_access_error(path: &Path, error: io::Error) -> CommandError {
    match error.kind() {
        io::ErrorKind::NotFound => CommandError::missing_path(
            "The requested path no longer exists.",
            Some(path.to_string_lossy().into_owned()),
        ),
        io::ErrorKind::PermissionDenied => CommandError::permission_denied(
            "Frogger does not have permission to read this folder.",
            Some(path.to_string_lossy().into_owned()),
        ),
        _ => CommandError::unavailable(
            "The requested folder is unavailable.",
            Some(format!("{}: {error}", path.display())),
        ),
    }
}

fn record_recent_path(conn: &Connection, path: &str) -> Result<(), CommandError> {
    let kind = if Path::new(path).is_dir() {
        "folder"
    } else {
        "file"
    };

    conn.execute(
        "INSERT INTO recents (id, path, kind, opened_at, open_count)
         VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT(path) DO UPDATE SET
            kind = excluded.kind,
            opened_at = excluded.opened_at,
            open_count = recents.open_count + 1",
        params![
            format!("recent-{}", Uuid::new_v4()),
            path,
            kind,
            Utc::now().to_rfc3339()
        ],
    )
    .map(|_| ())
    .map_err(anyhow::Error::from)
    .map_err(CommandError::from)
}

fn open_app_database(app: &tauri::AppHandle) -> Result<Connection, CommandError> {
    let database_path = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::internal(error.to_string()))?
        .join("frogger.sqlite3");
    persistence::open_database(&database_path).map_err(CommandError::from)
}

fn home_dir_string() -> Option<String> {
    directories::UserDirs::new().map(|dirs| dirs.home_dir().to_string_lossy().into_owned())
}

fn detect_file_access(home_dir: Option<&str>) -> FileAccessState {
    match home_dir {
        Some(path) if Path::new(path).is_dir() && Path::new(path).read_dir().is_ok() => {
            FileAccessState {
                status: FileAccessStatus::Granted,
                home_dir: Some(path.to_string()),
                message: None,
                recovery_hint: None,
            }
        }
        Some(path) => FileAccessState {
            status: FileAccessStatus::Denied,
            home_dir: Some(path.to_string()),
            message: Some("File access is required to browse your folders.".to_string()),
            recovery_hint: Some(recovery_hint()),
        },
        None => FileAccessState {
            status: FileAccessStatus::Denied,
            home_dir: None,
            message: Some("No home directory was detected for this user.".to_string()),
            recovery_hint: Some("Check the operating system account and filesystem permissions, then relaunch or retry.".to_string()),
        },
    }
}

fn recovery_hint() -> String {
    match std::env::consts::OS {
        "macos" => "Open System Settings, review Privacy & Security file access permissions, grant access, then retry.".to_string(),
        "windows" => "Check Windows security and controlled folder access settings, allow filesystem access, then retry.".to_string(),
        _ => "Check filesystem permissions for your home folder, grant access, then retry.".to_string(),
    }
}

fn load_settings(conn: &Connection) -> Result<AppSettings> {
    let raw = load_settings_map(conn)?;

    Ok(AppSettings {
        appearance_mode: match raw.get("appearance.mode").map(String::as_str) {
            Some("light") => AppearanceMode::Light,
            Some("dark") => AppearanceMode::Dark,
            _ => AppearanceMode::System,
        },
        hidden_files_visible: bool_setting(&raw, "browser.hiddenFilesVisible", false),
        file_extensions_visible: bool_setting(&raw, "browser.fileExtensionsVisible", false),
        folders_first: bool_setting(&raw, "browser.foldersFirst", true),
        path_bar_visible: bool_setting(&raw, "browser.pathBarVisible", true),
        restore_enabled: bool_setting(&raw, "restore.enabled", true),
        local_only_indexing: bool_setting(&raw, "privacy.localOnlyIndexing", true),
        previews_enabled: bool_setting(&raw, "previews.enabled", true),
        list_column_visibility: list_column_visibility(&raw),
        list_column_widths: list_column_widths(&raw),
        raw,
    })
}

fn load_settings_map(conn: &Connection) -> Result<BTreeMap<String, String>> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings ORDER BY key")
        .context("failed to prepare settings query")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .context("failed to query settings")?;

    let mut settings = BTreeMap::new();
    for row in rows {
        let (key, value) = row.context("failed to read setting row")?;
        settings.insert(key, value);
    }
    Ok(settings)
}

fn bool_setting(settings: &BTreeMap<String, String>, key: &str, default_value: bool) -> bool {
    settings
        .get(key)
        .map(|value| value == "true")
        .unwrap_or(default_value)
}

fn list_column_visibility(settings: &BTreeMap<String, String>) -> BTreeMap<String, bool> {
    ["name", "dateModified", "size", "kind"]
        .into_iter()
        .map(|column| {
            (
                column.to_string(),
                bool_setting(settings, &format!("list.column.{column}.visible"), true),
            )
        })
        .collect()
}

fn list_column_widths(settings: &BTreeMap<String, String>) -> BTreeMap<String, f64> {
    [
        ("name", 360.0_f64),
        ("dateModified", 205.0_f64),
        ("size", 120.0_f64),
        ("kind", 142.0_f64),
    ]
    .into_iter()
    .map(|(column, default_width)| {
        let width = settings
            .get(&format!("list.column.{column}.width"))
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(default_width)
            .clamp(72.0, 640.0);
        (column.to_string(), width)
    })
    .collect()
}

fn is_allowed_display_setting(key: &str, value: &str) -> bool {
    match key {
        "browser.hiddenFilesVisible" | "browser.fileExtensionsVisible" | "browser.foldersFirst" => {
            matches!(value, "true" | "false")
        }
        key if key.starts_with("list.column.") && key.ends_with(".visible") => {
            let column = key
                .trim_start_matches("list.column.")
                .trim_end_matches(".visible");
            matches!(column, "name" | "dateModified" | "size" | "kind")
                && matches!(value, "true" | "false")
                && (column != "name" || value == "true")
        }
        key if key.starts_with("list.column.") && key.ends_with(".width") => {
            let column = key
                .trim_start_matches("list.column.")
                .trim_end_matches(".width");
            matches!(column, "name" | "dateModified" | "size" | "kind")
                && value
                    .parse::<f64>()
                    .map(|width| (72.0..=640.0).contains(&width))
                    .unwrap_or(false)
        }
        _ => false,
    }
}

fn load_indexing_state(conn: &Connection) -> Result<IndexingState> {
    let (status, has_initial_index, checkpoint_json, error_json): (
        String,
        i64,
        String,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT status, has_initial_index, checkpoint_json, error_json
             FROM index_state WHERE id = 'metadata'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .context("failed to load metadata index state")?;

    let checkpoint = serde_json::from_str::<serde_json::Value>(&checkpoint_json).ok();
    let indexed_item_count = checkpoint
        .as_ref()
        .and_then(|value| value.get("indexedItemCount"))
        .and_then(|value| value.as_u64())
        .map(Ok)
        .unwrap_or_else(|| {
            conn.query_row("SELECT COUNT(*) FROM metadata_index", [], |row| {
                row.get::<_, u64>(0)
            })
            .context("failed to count indexed items")
        })?;

    let checkpoint_message = checkpoint
        .as_ref()
        .and_then(|value| value.get("message"))
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let error_message = error_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|value| {
            value
                .get("message")
                .and_then(|message| message.as_str())
                .map(ToString::to_string)
        });

    Ok(IndexingState {
        status: match status.as_str() {
            "initial_build" => IndexingStatus::InitialBuild,
            "reconciling" => IndexingStatus::Reconciling,
            "ready" => IndexingStatus::Ready,
            "failed" => IndexingStatus::Failed,
            _ => IndexingStatus::NotStarted,
        },
        has_initial_index: has_initial_index == 1,
        indexed_item_count,
        message: error_message.or(checkpoint_message),
    })
}

fn restore_windows(
    conn: &Connection,
    home_dir: &str,
    settings: &AppSettings,
) -> Result<Vec<WindowState>> {
    if !settings.restore_enabled {
        return Ok(vec![default_home_window(home_dir, settings)]);
    }

    let mut stmt = conn.prepare(
        "SELECT id, label, x, y, width, height, fullscreen, maximized, active_tab_id,
                sidebar_width, sidebar_collapsed
         FROM windows
         ORDER BY created_at, label",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(WindowState {
            id: row.get(0)?,
            label: row.get(1)?,
            geometry: WindowGeometry {
                x: row.get(2)?,
                y: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
                fullscreen: row.get::<_, i64>(6)? == 1,
                maximized: row.get::<_, i64>(7)? == 1,
            },
            active_tab_id: row.get(8)?,
            tabs: Vec::new(),
            sidebar_width: row.get(9)?,
            sidebar_collapsed: row.get::<_, i64>(10)? == 1,
        })
    })?;

    let mut windows = Vec::new();
    for row in rows {
        let mut window = row?;
        window.tabs = load_valid_tabs_for_window(conn, &window.id)?;
        if window.tabs.is_empty() {
            continue;
        }

        let active_tab_is_valid = window
            .active_tab_id
            .as_ref()
            .map(|active_tab_id| window.tabs.iter().any(|tab| &tab.id == active_tab_id))
            .unwrap_or(false);

        if !active_tab_is_valid {
            if let Some(first_tab) = window.tabs.first_mut() {
                first_tab.is_active = true;
                window.active_tab_id = Some(first_tab.id.clone());
            }
        }

        for tab in &mut window.tabs {
            tab.is_active = window
                .active_tab_id
                .as_ref()
                .map(|active_tab_id| active_tab_id == &tab.id)
                .unwrap_or(false);
        }

        windows.push(window);
    }

    if windows.is_empty() {
        windows.push(default_home_window(home_dir, settings));
    }

    Ok(windows)
}

fn load_folder_view_state(
    conn: &Connection,
    path: &str,
    settings: &AppSettings,
) -> Result<FolderViewState> {
    let mut stmt = conn.prepare(
        "SELECT view_mode, sort_key, sort_direction, folders_first, hidden_files_visible,
                file_extensions_visible, scroll_offset, selected_item_path
         FROM folder_view_states
         WHERE path = ?1",
    )?;

    let result = stmt.query_row([path], |row| {
        Ok(FolderViewState {
            view_mode: view_mode_from_db(&row.get::<_, String>(0)?),
            sort: SortState {
                key: sort_key_from_db(&row.get::<_, String>(1)?),
                direction: sort_direction_from_db(&row.get::<_, String>(2)?),
            },
            folders_first: row.get::<_, i64>(3)? == 1,
            hidden_files_visible: row.get::<_, i64>(4)? == 1,
            file_extensions_visible: row.get::<_, i64>(5)? == 1,
            scroll_offset: row.get(6)?,
            selected_item_path: row.get(7)?,
        })
    });

    match result {
        Ok(state) => Ok(state),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_folder_view_state(settings)),
        Err(error) => Err(error.into()),
    }
}

fn save_folder_view_state_impl(
    conn: &Connection,
    path: &str,
    state: &FolderViewState,
) -> Result<()> {
    conn.execute(
        "INSERT INTO folder_view_states (
            path, view_mode, sort_key, sort_direction, folders_first, hidden_files_visible,
            file_extensions_visible, scroll_offset, selected_item_path, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(path) DO UPDATE SET
            view_mode = excluded.view_mode,
            sort_key = excluded.sort_key,
            sort_direction = excluded.sort_direction,
            folders_first = excluded.folders_first,
            hidden_files_visible = excluded.hidden_files_visible,
            file_extensions_visible = excluded.file_extensions_visible,
            scroll_offset = excluded.scroll_offset,
            selected_item_path = excluded.selected_item_path,
            updated_at = excluded.updated_at",
        params![
            path,
            view_mode_to_db(&state.view_mode),
            sort_key_to_db(&state.sort.key),
            sort_direction_to_db(&state.sort.direction),
            bool_to_i64(state.folders_first),
            bool_to_i64(state.hidden_files_visible),
            bool_to_i64(state.file_extensions_visible),
            state.scroll_offset,
            state.selected_item_path,
        ],
    )?;
    Ok(())
}

fn default_folder_view_state(settings: &AppSettings) -> FolderViewState {
    FolderViewState {
        view_mode: ViewMode::List,
        sort: SortState {
            key: SortKey::Name,
            direction: SortDirection::Asc,
        },
        folders_first: settings.folders_first,
        hidden_files_visible: settings.hidden_files_visible,
        file_extensions_visible: settings.file_extensions_visible,
        scroll_offset: 0.0,
        selected_item_path: None,
    }
}

fn load_valid_tabs_for_window(conn: &Connection, window_id: &str) -> Result<Vec<TabState>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, title, position, is_active, view_mode, sort_key, sort_direction,
                folders_first, hidden_files_visible, file_extensions_visible, scroll_offset,
                selected_item_path
         FROM tabs
         WHERE window_id = ?1
         ORDER BY position, created_at",
    )?;

    let rows = stmt.query_map([window_id], |row| {
        let selected_item_path: Option<String> = row.get(12)?;
        Ok(TabState {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            position: row.get(3)?,
            is_active: row.get::<_, i64>(4)? == 1,
            folder_state: FolderViewState {
                view_mode: view_mode_from_db(&row.get::<_, String>(5)?),
                sort: SortState {
                    key: sort_key_from_db(&row.get::<_, String>(6)?),
                    direction: sort_direction_from_db(&row.get::<_, String>(7)?),
                },
                folders_first: row.get::<_, i64>(8)? == 1,
                hidden_files_visible: row.get::<_, i64>(9)? == 1,
                file_extensions_visible: row.get::<_, i64>(10)? == 1,
                scroll_offset: row.get(11)?,
                selected_item_path,
            },
        })
    })?;

    let mut tabs = Vec::new();
    for row in rows {
        let mut tab = row?;
        if !is_recents_virtual_path(&tab.path) && !Path::new(&tab.path).is_dir() {
            continue;
        }

        if let Some(selected_item_path) = tab.folder_state.selected_item_path.as_deref() {
            if !Path::new(selected_item_path).exists() {
                tab.folder_state.selected_item_path = None;
            }
        }

        tabs.push(tab);
    }

    Ok(tabs)
}

fn save_windows(conn: &mut Connection, windows: &[WindowState]) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM tabs", [])?;
    tx.execute("DELETE FROM windows", [])?;

    for window in windows {
        tx.execute(
            "INSERT INTO windows (
                id, label, x, y, width, height, fullscreen, maximized, active_tab_id,
                sidebar_width, sidebar_collapsed
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                window.id,
                window.label,
                window.geometry.x,
                window.geometry.y,
                window.geometry.width,
                window.geometry.height,
                bool_to_i64(window.geometry.fullscreen),
                bool_to_i64(window.geometry.maximized),
                window.active_tab_id,
                window.sidebar_width,
                bool_to_i64(window.sidebar_collapsed),
            ],
        )?;

        for tab in &window.tabs {
            tx.execute(
                "INSERT INTO tabs (
                    id, window_id, path, title, position, is_active, view_mode, sort_key,
                    sort_direction, folders_first, hidden_files_visible, file_extensions_visible,
                    scroll_offset, selected_item_path
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    tab.id,
                    window.id,
                    tab.path,
                    tab.title,
                    tab.position,
                    bool_to_i64(tab.is_active),
                    view_mode_to_db(&tab.folder_state.view_mode),
                    sort_key_to_db(&tab.folder_state.sort.key),
                    sort_direction_to_db(&tab.folder_state.sort.direction),
                    bool_to_i64(tab.folder_state.folders_first),
                    bool_to_i64(tab.folder_state.hidden_files_visible),
                    bool_to_i64(tab.folder_state.file_extensions_visible),
                    tab.folder_state.scroll_offset,
                    tab.folder_state.selected_item_path,
                ],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

fn view_mode_from_db(value: &str) -> ViewMode {
    match value {
        "grid" => ViewMode::Grid,
        "column" => ViewMode::Column,
        "gallery" => ViewMode::Gallery,
        _ => ViewMode::List,
    }
}

fn view_mode_to_db(value: &ViewMode) -> &'static str {
    match value {
        ViewMode::List => "list",
        ViewMode::Grid => "grid",
        ViewMode::Column => "column",
        ViewMode::Gallery => "gallery",
    }
}

fn sort_key_from_db(value: &str) -> SortKey {
    match value {
        "date_modified" => SortKey::DateModified,
        "size" => SortKey::Size,
        "kind" => SortKey::Kind,
        "path" => SortKey::Path,
        _ => SortKey::Name,
    }
}

fn sort_key_to_db(value: &SortKey) -> &'static str {
    match value {
        SortKey::Name => "name",
        SortKey::DateModified => "date_modified",
        SortKey::Size => "size",
        SortKey::Kind => "kind",
        SortKey::Path => "path",
    }
}

fn sort_direction_from_db(value: &str) -> SortDirection {
    match value {
        "desc" => SortDirection::Desc,
        _ => SortDirection::Asc,
    }
}

fn sort_direction_to_db(value: &SortDirection) -> &'static str {
    match value {
        SortDirection::Asc => "asc",
        SortDirection::Desc => "desc",
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn default_home_window(home_dir: &str, settings: &AppSettings) -> WindowState {
    window_for_path(
        "main-window".to_string(),
        "main".to_string(),
        "home-tab".to_string(),
        home_dir,
        "Home".to_string(),
        settings,
    )
}

fn window_for_path(
    id: String,
    label: String,
    tab_id: String,
    path: &str,
    title: String,
    settings: &AppSettings,
) -> WindowState {
    WindowState {
        id,
        label,
        geometry: WindowGeometry {
            x: None,
            y: None,
            width: 1200.0,
            height: 780.0,
            fullscreen: false,
            maximized: false,
        },
        active_tab_id: Some(tab_id.clone()),
        tabs: vec![TabState {
            id: tab_id,
            path: path.to_string(),
            title,
            position: 0,
            is_active: true,
            folder_state: default_folder_view_state(settings),
        }],
        sidebar_width: 240.0,
        sidebar_collapsed: false,
    }
}

fn folder_title(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "Home".to_string())
}

fn load_sidebar_state(conn: &Connection, home_dir: Option<String>) -> Result<SidebarState> {
    Ok(SidebarState {
        sections: load_sidebar_sections(conn)?,
        recent_items: load_recent_items(conn)?,
        favorites: load_favorites(conn)?,
        locations: detect_locations(home_dir),
        recents_virtual_folder_id: "recents".to_string(),
    })
}

fn load_sidebar_sections(conn: &Connection) -> Result<Vec<SidebarSectionState>> {
    let mut stmt =
        conn.prepare("SELECT id, visible, position FROM sidebar_sections ORDER BY position, id")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut sections = Vec::new();
    for row in rows {
        let (id, visible, position) = row?;
        if let Some(section_id) = sidebar_section_from_db(&id) {
            sections.push(SidebarSectionState {
                label: sidebar_section_label(&section_id).to_string(),
                id: section_id,
                visible: visible == 1,
                position,
            });
        }
    }

    if sections.is_empty() {
        sections = vec![
            SidebarSectionState {
                id: SidebarSectionId::Recents,
                label: "Recents".to_string(),
                visible: true,
                position: 0,
            },
            SidebarSectionState {
                id: SidebarSectionId::Favorites,
                label: "Favorites".to_string(),
                visible: true,
                position: 1,
            },
            SidebarSectionState {
                id: SidebarSectionId::Locations,
                label: "Locations".to_string(),
                visible: true,
                position: 2,
            },
        ];
    }

    Ok(sections)
}

fn load_recent_items(conn: &Connection) -> Result<Vec<SidebarItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, path FROM recents ORDER BY opened_at DESC, open_count DESC LIMIT 20",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut items = Vec::new();
    for row in rows {
        let (id, path) = row?;
        if Path::new(&path).exists() {
            items.push(SidebarItem {
                id,
                label: folder_title(&path),
                path,
                item_type: SidebarItemType::Recent,
            });
        }
    }

    Ok(items)
}

fn load_favorites(conn: &Connection) -> Result<Vec<SidebarItem>> {
    let mut stmt =
        conn.prepare("SELECT id, path, label FROM favorites ORDER BY position, label")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut items = Vec::new();
    for row in rows {
        let (id, path, label) = row?;
        if Path::new(&path).is_dir() {
            items.push(SidebarItem {
                id,
                label,
                path,
                item_type: SidebarItemType::Favorite,
            });
        }
    }

    Ok(items)
}

fn detect_locations(home_dir: Option<String>) -> Vec<SidebarItem> {
    let mut locations_by_path: BTreeMap<String, SidebarItem> = BTreeMap::new();

    if let Some(home) = home_dir {
        push_location(
            &mut locations_by_path,
            "home".to_string(),
            "Home".to_string(),
            home.clone(),
            SidebarItemType::Home,
        );

        let home_path = PathBuf::from(home);
        for cloud_name in ["Dropbox", "Google Drive", "OneDrive", "iCloud Drive"] {
            let candidate = home_path.join(cloud_name);
            if candidate.is_dir() {
                push_location(
                    &mut locations_by_path,
                    format!("cloud-{}", cloud_name.to_lowercase().replace(' ', "-")),
                    cloud_name.to_string(),
                    candidate.to_string_lossy().into_owned(),
                    SidebarItemType::CloudFolder,
                );
            }
        }

        let cloud_storage = home_path.join("Library").join("CloudStorage");
        if let Ok(entries) = std::fs::read_dir(cloud_storage) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let label = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(clean_cloud_folder_label)
                        .unwrap_or_else(|| "Cloud Folder".to_string());
                    push_location(
                        &mut locations_by_path,
                        format!("cloud-{}", stable_id_from_path(&path)),
                        label,
                        path.to_string_lossy().into_owned(),
                        SidebarItemType::CloudFolder,
                    );
                }
            }
        }
    }

    for mounted_root in mounted_roots() {
        if let Ok(entries) = std::fs::read_dir(&mounted_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let label = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("Drive")
                        .to_string();
                    push_location(
                        &mut locations_by_path,
                        format!("drive-{}", stable_id_from_path(&path)),
                        label,
                        path.to_string_lossy().into_owned(),
                        SidebarItemType::Drive,
                    );
                }
            }
        }
    }

    locations_by_path.into_values().collect()
}

fn push_location(
    locations_by_path: &mut BTreeMap<String, SidebarItem>,
    id: String,
    label: String,
    path: String,
    item_type: SidebarItemType,
) {
    locations_by_path
        .entry(path.clone())
        .or_insert(SidebarItem {
            id,
            label,
            path,
            item_type,
        });
}

fn mounted_roots() -> Vec<PathBuf> {
    match std::env::consts::OS {
        "macos" => vec![PathBuf::from("/Volumes")],
        "linux" => vec![PathBuf::from("/mnt"), PathBuf::from("/media")],
        _ => Vec::new(),
    }
}

fn clean_cloud_folder_label(raw: &str) -> String {
    raw.replace("CloudStorage", "")
        .replace('-', " ")
        .trim()
        .to_string()
}

fn stable_id_from_path(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn sidebar_section_from_db(value: &str) -> Option<SidebarSectionId> {
    match value {
        "recents" => Some(SidebarSectionId::Recents),
        "favorites" => Some(SidebarSectionId::Favorites),
        "locations" => Some(SidebarSectionId::Locations),
        _ => None,
    }
}

fn sidebar_section_to_db(value: &SidebarSectionId) -> &'static str {
    match value {
        SidebarSectionId::Recents => "recents",
        SidebarSectionId::Favorites => "favorites",
        SidebarSectionId::Locations => "locations",
    }
}

fn sidebar_section_label(value: &SidebarSectionId) -> &'static str {
    match value {
        SidebarSectionId::Recents => "Recents",
        SidebarSectionId::Favorites => "Favorites",
        SidebarSectionId::Locations => "Locations",
    }
}

fn build_capabilities() -> AppCapabilities {
    AppCapabilities {
        native_titlebar_tabs: cfg!(target_os = "macos"),
        open_with_chooser: true,
        reliable_trash_undo: false,
        outbound_file_drag: true,
        cloud_placeholder_detection: cfg!(any(target_os = "macos", target_os = "windows")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn load_settings_reads_defaults_from_migrated_database() {
        let path = std::env::temp_dir().join(format!("frogger-command-{}.sqlite3", Uuid::new_v4()));
        let conn = persistence::open_database(&path).expect("database should migrate");

        let settings = load_settings(&conn).expect("settings should load");
        assert_eq!(settings.appearance_mode, AppearanceMode::System);
        assert!(!settings.hidden_files_visible);
        assert!(!settings.file_extensions_visible);
        assert!(settings.folders_first);
        assert!(settings.path_bar_visible);
        assert!(settings.restore_enabled);
        assert!(settings.local_only_indexing);
        assert!(settings.previews_enabled);
        assert_eq!(settings.list_column_visibility.get("name"), Some(&true));
        assert_eq!(
            settings.list_column_visibility.get("dateModified"),
            Some(&true)
        );
        assert_eq!(settings.list_column_widths.get("name"), Some(&360.0));

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn load_indexing_state_counts_metadata_rows() {
        let path =
            std::env::temp_dir().join(format!("frogger-indexing-{}.sqlite3", Uuid::new_v4()));
        let conn = persistence::open_database(&path).expect("database should migrate");

        conn.execute(
            "INSERT INTO metadata_index (
                path, parent_path, name, display_name, kind, is_dir, search_text
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params!["/tmp/a", "/tmp", "a", "a", "Folder", 1_i64, "a folder"],
        )
        .expect("metadata row should insert");
        conn.execute(
            "UPDATE index_state SET status = 'ready', has_initial_index = 1 WHERE id = 'metadata'",
            [],
        )
        .expect("index state should update");

        let state = load_indexing_state(&conn).expect("state should load");
        assert_eq!(state.status, IndexingStatus::Ready);
        assert!(state.has_initial_index);
        assert_eq!(state.indexed_item_count, 1);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn search_metadata_ranks_exact_prefix_and_folders() {
        let path = std::env::temp_dir().join(format!("frogger-search-{}.sqlite3", Uuid::new_v4()));
        let conn = persistence::open_database(&path).expect("database should migrate");

        insert_metadata_search_row(
            &conn,
            TestMetadataRow {
                path: "/tmp/project",
                parent_path: "/tmp",
                name: "project",
                display_name: "project",
                kind: "Folder",
                is_dir: true,
                size: None,
                search_text: "project folder tmp",
            },
        );
        insert_metadata_search_row(
            &conn,
            TestMetadataRow {
                path: "/tmp/project-notes.md",
                parent_path: "/tmp",
                name: "project-notes.md",
                display_name: "project-notes",
                kind: "Markdown Document",
                is_dir: false,
                size: Some(42),
                search_text: "project-notes markdown document tmp",
            },
        );
        insert_metadata_search_row(
            &conn,
            TestMetadataRow {
                path: "/tmp/other.txt",
                parent_path: "/tmp",
                name: "other.txt",
                display_name: "other",
                kind: "Text Document",
                is_dir: false,
                size: Some(5),
                search_text: "other text document tmp",
            },
        );

        let results = search_metadata_impl(&conn, "project", Some(10)).expect("search should run");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "/tmp/project");
        assert_eq!(results[0].match_reason, SearchMatchReason::Exact);
        assert!(results[0].is_dir);
        assert_eq!(results[1].match_reason, SearchMatchReason::Prefix);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn search_metadata_supports_fuzzy_filename_matches() {
        let path =
            std::env::temp_dir().join(format!("frogger-search-fuzzy-{}.sqlite3", Uuid::new_v4()));
        let conn = persistence::open_database(&path).expect("database should migrate");

        insert_metadata_search_row(
            &conn,
            TestMetadataRow {
                path: "/tmp/README.md",
                parent_path: "/tmp",
                name: "README.md",
                display_name: "README",
                kind: "Markdown Document",
                is_dir: false,
                size: Some(100),
                search_text: "readme markdown document tmp",
            },
        );

        let results = search_metadata_impl(&conn, "rdm", Some(10)).expect("search should run");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "/tmp/README.md");
        assert_eq!(results[0].match_reason, SearchMatchReason::Fuzzy);

        std::fs::remove_file(path).ok();
    }

    struct TestMetadataRow<'a> {
        path: &'a str,
        parent_path: &'a str,
        name: &'a str,
        display_name: &'a str,
        kind: &'a str,
        is_dir: bool,
        size: Option<i64>,
        search_text: &'a str,
    }

    fn insert_metadata_search_row(conn: &Connection, row: TestMetadataRow<'_>) {
        conn.execute(
            "INSERT INTO metadata_index (
                path, parent_path, name, display_name, kind, is_dir, size, search_text
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.path,
                row.parent_path,
                row.name,
                row.display_name,
                row.kind,
                if row.is_dir { 1_i64 } else { 0_i64 },
                row.size,
                row.search_text,
            ],
        )
        .expect("metadata search row should insert");
    }

    #[test]
    fn file_access_denied_when_home_is_missing() {
        let access = detect_file_access(Some("/definitely/not/a/frogger/home"));
        assert_eq!(access.status, FileAccessStatus::Denied);
        assert!(access.recovery_hint.is_some());
    }

    fn test_settings() -> AppSettings {
        AppSettings {
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
        }
    }

    #[test]
    fn default_home_window_uses_phase_one_first_launch_defaults() {
        let settings = test_settings();

        let window = default_home_window("/Users/example", &settings);
        assert_eq!(window.geometry.width, 1200.0);
        assert_eq!(window.geometry.height, 780.0);
        assert_eq!(window.sidebar_width, 240.0);
        assert!(!window.sidebar_collapsed);
        assert_eq!(window.active_tab_id.as_deref(), Some("home-tab"));
        assert_eq!(window.tabs.len(), 1);

        let tab = &window.tabs[0];
        assert_eq!(tab.path, "/Users/example");
        assert_eq!(tab.title, "Home");
        assert_eq!(tab.folder_state.view_mode, ViewMode::List);
        assert_eq!(tab.folder_state.sort.key, SortKey::Name);
        assert_eq!(tab.folder_state.sort.direction, SortDirection::Asc);
        assert!(tab.folder_state.folders_first);
        assert!(!tab.folder_state.hidden_files_visible);
        assert!(!tab.folder_state.file_extensions_visible);
    }

    #[test]
    fn restore_windows_drops_unavailable_tabs_and_keeps_valid_state() {
        let database_path =
            std::env::temp_dir().join(format!("frogger-restore-valid-{}.sqlite3", Uuid::new_v4()));
        let conn = persistence::open_database(&database_path).expect("database should migrate");
        let temp = tempdir().expect("tempdir should exist");
        let valid_dir = temp.path().join("valid");
        let selected_file = valid_dir.join("selected.txt");
        let invalid_dir = temp.path().join("missing");
        std::fs::create_dir_all(&valid_dir).expect("valid dir should be created");
        std::fs::write(&selected_file, "selected").expect("selected file should be created");

        conn.execute(
            "INSERT INTO windows (
                id, label, x, y, width, height, active_tab_id, sidebar_width, sidebar_collapsed
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "window-1",
                "main",
                11.0_f64,
                12.0_f64,
                1000.0_f64,
                700.0_f64,
                "tab-valid",
                288.0_f64,
                0_i64
            ],
        )
        .expect("window should insert");
        conn.execute(
            "INSERT INTO tabs (
                id, window_id, path, title, position, is_active, view_mode, sort_key,
                sort_direction, folders_first, hidden_files_visible, file_extensions_visible,
                scroll_offset, selected_item_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                "tab-valid",
                "window-1",
                valid_dir.to_string_lossy().as_ref(),
                "valid",
                0_i64,
                1_i64,
                "grid",
                "date_modified",
                "desc",
                1_i64,
                1_i64,
                1_i64,
                432.0_f64,
                selected_file.to_string_lossy().as_ref()
            ],
        )
        .expect("valid tab should insert");
        conn.execute(
            "INSERT INTO tabs (
                id, window_id, path, title, position, is_active
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "tab-invalid",
                "window-1",
                invalid_dir.to_string_lossy().as_ref(),
                "missing",
                1_i64,
                0_i64
            ],
        )
        .expect("invalid tab should insert");

        let windows = restore_windows(
            &conn,
            temp.path().to_string_lossy().as_ref(),
            &test_settings(),
        )
        .expect("windows should restore");

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].tabs.len(), 1);
        assert_eq!(windows[0].sidebar_width, 288.0);
        assert_eq!(windows[0].active_tab_id.as_deref(), Some("tab-valid"));
        assert_eq!(windows[0].tabs[0].path, valid_dir.to_string_lossy());
        assert!(windows[0].tabs[0].is_active);
        assert_eq!(windows[0].tabs[0].folder_state.view_mode, ViewMode::Grid);
        assert_eq!(
            windows[0].tabs[0].folder_state.sort.key,
            SortKey::DateModified
        );
        assert_eq!(
            windows[0].tabs[0].folder_state.sort.direction,
            SortDirection::Desc
        );
        assert!(windows[0].tabs[0].folder_state.hidden_files_visible);
        assert!(windows[0].tabs[0].folder_state.file_extensions_visible);
        assert_eq!(windows[0].tabs[0].folder_state.scroll_offset, 432.0);
        assert_eq!(
            windows[0].tabs[0]
                .folder_state
                .selected_item_path
                .as_deref(),
            Some(selected_file.to_string_lossy().as_ref())
        );

        std::fs::remove_file(database_path).ok();
    }

    #[test]
    fn restore_windows_falls_back_to_home_when_no_valid_tabs_remain() {
        let database_path = std::env::temp_dir().join(format!(
            "frogger-restore-fallback-{}.sqlite3",
            Uuid::new_v4()
        ));
        let conn = persistence::open_database(&database_path).expect("database should migrate");
        let temp = tempdir().expect("tempdir should exist");
        let missing = temp.path().join("missing");

        conn.execute(
            "INSERT INTO windows (id, label, width, height, active_tab_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["window-1", "main", 1000.0_f64, 700.0_f64, "tab-missing"],
        )
        .expect("window should insert");
        conn.execute(
            "INSERT INTO tabs (id, window_id, path, title, position, is_active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "tab-missing",
                "window-1",
                missing.to_string_lossy().as_ref(),
                "missing",
                0_i64,
                1_i64
            ],
        )
        .expect("tab should insert");

        let windows = restore_windows(
            &conn,
            temp.path().to_string_lossy().as_ref(),
            &test_settings(),
        )
        .expect("fallback should restore");

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].tabs.len(), 1);
        assert_eq!(windows[0].tabs[0].path, temp.path().to_string_lossy());
        assert_eq!(windows[0].tabs[0].title, "Home");
        assert_eq!(windows[0].tabs[0].folder_state.view_mode, ViewMode::List);
        assert_eq!(windows[0].tabs[0].folder_state.sort.key, SortKey::Name);
        assert_eq!(
            windows[0].tabs[0].folder_state.sort.direction,
            SortDirection::Asc
        );

        std::fs::remove_file(database_path).ok();
    }

    #[test]
    fn list_directory_hides_dotfiles_and_keeps_regular_folders_visible() {
        let temp = tempdir().expect("tempdir should exist");
        std::fs::write(temp.path().join("alpha.txt"), "alpha").expect("file should be written");
        std::fs::write(temp.path().join(".secret"), "secret").expect("dotfile should be written");
        std::fs::create_dir(temp.path().join("node_modules"))
            .expect("dependency folder should be created");

        let listing = list_directory_impl(
            temp.path().to_string_lossy().into_owned(),
            &SortState {
                key: SortKey::Name,
                direction: SortDirection::Asc,
            },
            true,
            false,
            false,
            None,
            None,
        )
        .expect("directory should list");

        assert_eq!(listing.total_count, 2);
        assert!(listing.loading_complete);
        assert!(listing.next_cursor.is_none());
        assert!(listing
            .entries
            .iter()
            .any(|entry| entry.name == "alpha.txt"));
        assert!(listing
            .entries
            .iter()
            .any(|entry| entry.name == "node_modules"));
        assert!(!listing.entries.iter().any(|entry| entry.name == ".secret"));
        assert_eq!(listing.entries[0].name, "node_modules");
    }

    #[test]
    fn list_directory_can_include_hidden_files_and_metadata() {
        let temp = tempdir().expect("tempdir should exist");
        let file = temp.path().join("notes.md");
        std::fs::write(&file, "hello").expect("file should be written");
        std::fs::write(temp.path().join(".env"), "KEY=value").expect("hidden file should write");

        let listing = list_directory_impl(
            temp.path().to_string_lossy().into_owned(),
            &SortState {
                key: SortKey::Name,
                direction: SortDirection::Asc,
            },
            false,
            true,
            true,
            None,
            Some(1),
        )
        .expect("directory should list");

        assert_eq!(listing.total_count, 2);
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.next_cursor.as_deref(), Some("1"));

        let full_listing = list_directory_impl(
            temp.path().to_string_lossy().into_owned(),
            &SortState {
                key: SortKey::Name,
                direction: SortDirection::Asc,
            },
            false,
            true,
            true,
            None,
            None,
        )
        .expect("directory should list");
        let notes = full_listing
            .entries
            .iter()
            .find(|entry| entry.name == "notes.md")
            .expect("notes should exist");
        assert_eq!(notes.extension.as_deref(), Some("md"));
        assert_eq!(notes.display_name, "notes.md");
        assert_eq!(notes.kind, "Markdown Document");
        assert_eq!(notes.icon.name, "markdown");
        assert_eq!(notes.size, Some(5));
        assert!(notes.modified_at.is_some());
        assert!(full_listing
            .entries
            .iter()
            .any(|entry| entry.name == ".env" && entry.hidden));
    }

    #[test]
    fn list_directory_assigns_document_icon_categories() {
        let temp = tempdir().expect("tempdir should exist");
        for name in [
            "archive.zip",
            "budget.xlsx",
            "data.csv",
            "letter.docx",
            "notes.md",
            "readme.txt",
            "report.pdf",
            "unknown.blob",
        ] {
            std::fs::write(temp.path().join(name), "sample").expect("file should be written");
        }

        let listing = list_directory_impl(
            temp.path().to_string_lossy().into_owned(),
            &SortState {
                key: SortKey::Name,
                direction: SortDirection::Asc,
            },
            false,
            true,
            true,
            None,
            None,
        )
        .expect("directory should list");
        let entries = listing
            .entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(entries["archive.zip"].icon.name, "archive");
        assert_eq!(entries["budget.xlsx"].icon.name, "spreadsheet");
        assert_eq!(entries["data.csv"].icon.name, "spreadsheet");
        assert_eq!(entries["letter.docx"].icon.name, "word-document");
        assert_eq!(entries["notes.md"].icon.name, "markdown");
        assert_eq!(entries["report.pdf"].icon.name, "pdf");
        assert_eq!(entries["unknown.blob"].icon.name, "generic");
        assert_eq!(entries["readme.txt"].icon.name, "file");
        assert_eq!(entries["unknown.blob"].kind, "Document");
    }

    #[test]
    fn list_directory_returns_recoverable_missing_path_error() {
        let missing = std::env::temp_dir().join(format!("frogger-missing-{}", Uuid::new_v4()));
        let error = list_directory_impl(
            missing.to_string_lossy().into_owned(),
            &SortState {
                key: SortKey::Name,
                direction: SortDirection::Asc,
            },
            true,
            false,
            false,
            None,
            None,
        )
        .expect_err("missing path should error");

        assert_eq!(error.code, "missing_path");
        assert!(error.recoverable);
    }

    #[test]
    fn display_settings_and_folder_view_state_persist() {
        let database_path = std::env::temp_dir().join(format!(
            "frogger-display-settings-{}.sqlite3",
            Uuid::new_v4()
        ));
        let conn = persistence::open_database(&database_path).expect("database should migrate");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params!["list.column.size.visible", "false"],
        )
        .expect("setting should upsert");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params!["list.column.name.width", "420"],
        )
        .expect("width should upsert");

        let settings = load_settings(&conn).expect("settings should load");
        assert_eq!(settings.list_column_visibility.get("size"), Some(&false));
        assert_eq!(settings.list_column_widths.get("name"), Some(&420.0));
        assert!(is_allowed_display_setting(
            "browser.hiddenFilesVisible",
            "true"
        ));
        assert!(!is_allowed_display_setting(
            "list.column.name.visible",
            "false"
        ));

        let state = FolderViewState {
            view_mode: ViewMode::List,
            sort: SortState {
                key: SortKey::Size,
                direction: SortDirection::Desc,
            },
            folders_first: false,
            hidden_files_visible: true,
            file_extensions_visible: true,
            scroll_offset: 128.0,
            selected_item_path: Some("/tmp/example.txt".to_string()),
        };
        save_folder_view_state_impl(&conn, "/tmp", &state).expect("folder state should save");
        let loaded =
            load_folder_view_state(&conn, "/tmp", &settings).expect("folder state should load");
        assert_eq!(loaded.sort.key, SortKey::Size);
        assert_eq!(loaded.sort.direction, SortDirection::Desc);
        assert!(!loaded.folders_first);
        assert!(loaded.hidden_files_visible);
        assert_eq!(loaded.scroll_offset, 128.0);

        std::fs::remove_file(database_path).ok();
    }

    #[test]
    fn recents_insert_once_and_sort_by_latest_open() {
        let database_path =
            std::env::temp_dir().join(format!("frogger-recents-{}.sqlite3", Uuid::new_v4()));
        let conn = persistence::open_database(&database_path).expect("database should migrate");
        let temp = tempdir().expect("tempdir should exist");
        let older = temp.path().join("older.txt");
        let newer = temp.path().join("newer.txt");
        std::fs::write(&older, "older").expect("older file should be written");
        std::fs::write(&newer, "newer").expect("newer file should be written");

        record_recent_path(&conn, older.to_string_lossy().as_ref()).expect("older should record");
        record_recent_path(&conn, newer.to_string_lossy().as_ref()).expect("newer should record");
        conn.execute(
            "UPDATE recents SET opened_at = ?1 WHERE path = ?2",
            params!["2024-01-01T00:00:00Z", older.to_string_lossy().as_ref()],
        )
        .expect("older timestamp should update");
        conn.execute(
            "UPDATE recents SET opened_at = ?1 WHERE path = ?2",
            params!["2024-02-01T00:00:00Z", newer.to_string_lossy().as_ref()],
        )
        .expect("newer timestamp should update");
        record_recent_path(&conn, older.to_string_lossy().as_ref()).expect("older should update");

        let open_count: i64 = conn
            .query_row(
                "SELECT open_count FROM recents WHERE path = ?1",
                [older.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .expect("open count should load");
        assert_eq!(open_count, 2);

        let sidebar_items = load_recent_items(&conn).expect("recents should load");
        assert_eq!(sidebar_items.len(), 2);
        assert_eq!(sidebar_items[0].path, older.to_string_lossy());

        std::fs::remove_file(database_path).ok();
    }

    #[test]
    fn recents_virtual_folder_returns_existing_items_in_descending_order() {
        let database_path = std::env::temp_dir().join(format!(
            "frogger-recents-virtual-{}.sqlite3",
            Uuid::new_v4()
        ));
        let conn = persistence::open_database(&database_path).expect("database should migrate");
        let temp = tempdir().expect("tempdir should exist");
        let first = temp.path().join("first.txt");
        let second = temp.path().join("second.txt");
        let missing = temp.path().join("missing.txt");
        std::fs::write(&first, "first").expect("first file should be written");
        std::fs::write(&second, "second").expect("second file should be written");

        for (path, opened_at) in [
            (first.to_string_lossy().into_owned(), "2024-01-01T00:00:00Z"),
            (
                second.to_string_lossy().into_owned(),
                "2024-02-01T00:00:00Z",
            ),
            (
                missing.to_string_lossy().into_owned(),
                "2024-03-01T00:00:00Z",
            ),
        ] {
            conn.execute(
                "INSERT INTO recents (id, path, kind, opened_at, open_count)
                 VALUES (?1, ?2, 'file', ?3, 1)",
                params![format!("recent-{}", Uuid::new_v4()), path, opened_at],
            )
            .expect("recent should insert");
        }

        let listing = list_recents_directory_impl(&conn, false, true, None, None)
            .expect("virtual recents should list");
        assert_eq!(listing.path, RECENTS_VIRTUAL_PATH);
        assert_eq!(listing.total_count, 2);
        assert_eq!(listing.entries[0].name, "second.txt");
        assert_eq!(listing.entries[1].name, "first.txt");

        std::fs::remove_file(database_path).ok();
    }

    #[test]
    fn thumbnail_cache_reuses_and_invalidates_metadata() {
        let database_path =
            std::env::temp_dir().join(format!("frogger-thumbnails-{}.sqlite3", Uuid::new_v4()));
        let conn = persistence::open_database(&database_path).expect("database should migrate");
        let temp = tempdir().expect("tempdir should exist");
        let cache_dir = temp.path().join("cache");
        let image_path = temp.path().join("sample.png");
        let image = image::RgbImage::from_pixel(16, 16, image::Rgb([20, 120, 220]));
        image.save(&image_path).expect("image should save");
        let metadata = std::fs::metadata(&image_path).expect("image metadata should read");
        let modified_at = metadata.modified().ok().map(system_time_to_rfc3339);

        let generated = get_or_generate_thumbnail(
            &conn,
            &cache_dir,
            &image_path,
            metadata.len(),
            modified_at.as_deref(),
        )
        .expect("thumbnail generation should succeed")
        .expect("thumbnail should exist");
        assert!(!generated.cache_hit);
        assert!(Path::new(&generated.thumbnail_path).is_file());

        let cached = get_or_generate_thumbnail(
            &conn,
            &cache_dir,
            &image_path,
            metadata.len(),
            modified_at.as_deref(),
        )
        .expect("thumbnail cache lookup should succeed")
        .expect("thumbnail should exist");
        assert!(cached.cache_hit);
        assert_eq!(cached.thumbnail_path, generated.thumbnail_path);

        let invalidated = get_or_generate_thumbnail(
            &conn,
            &cache_dir,
            &image_path,
            metadata.len() + 1,
            modified_at.as_deref(),
        )
        .expect("thumbnail regeneration should succeed")
        .expect("thumbnail should exist");
        assert!(!invalidated.cache_hit);

        std::fs::remove_file(database_path).ok();
    }

    #[test]
    fn thumbnail_cleanup_removes_orphaned_metadata() {
        let database_path = std::env::temp_dir().join(format!(
            "frogger-thumbnail-cleanup-{}.sqlite3",
            Uuid::new_v4()
        ));
        let conn = persistence::open_database(&database_path).expect("database should migrate");
        conn.execute(
            "INSERT INTO thumbnail_metadata (
                source_path, thumbnail_path, source_size, width, height
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "/tmp/frogger-missing-source.png",
                "/tmp/frogger-missing-thumb.png",
                1_u64,
                10_u32,
                10_u32
            ],
        )
        .expect("thumbnail metadata should insert");

        let removed = cleanup_stale_thumbnail_metadata(&conn).expect("cleanup should run");
        assert_eq!(removed, 1);
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM thumbnail_metadata", [], |row| {
                row.get(0)
            })
            .expect("count should read");
        assert_eq!(remaining, 0);

        std::fs::remove_file(database_path).ok();
    }

    #[test]
    fn save_windows_round_trips_session_state() {
        let database_path =
            std::env::temp_dir().join(format!("frogger-save-session-{}.sqlite3", Uuid::new_v4()));
        let mut conn = persistence::open_database(&database_path).expect("database should migrate");
        let temp = tempdir().expect("tempdir should exist");
        let folder = temp.path().join("roundtrip");
        std::fs::create_dir_all(&folder).expect("folder should be created");

        let mut window = default_home_window(folder.to_string_lossy().as_ref(), &test_settings());
        window.id = "saved-window".to_string();
        window.label = "saved".to_string();
        window.geometry.x = Some(20.0);
        window.geometry.y = Some(30.0);
        window.sidebar_width = 312.0;
        window.tabs[0].id = "saved-tab".to_string();
        window.tabs[0].title = "roundtrip".to_string();
        window.active_tab_id = Some("saved-tab".to_string());
        window.tabs[0].folder_state.view_mode = ViewMode::Gallery;
        window.tabs[0].folder_state.sort.key = SortKey::Kind;
        window.tabs[0].folder_state.sort.direction = SortDirection::Desc;

        save_windows(&mut conn, &[window]).expect("session should save");
        let restored = restore_windows(
            &conn,
            temp.path().to_string_lossy().as_ref(),
            &test_settings(),
        )
        .expect("session should restore");

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, "saved-window");
        assert_eq!(restored[0].geometry.x, Some(20.0));
        assert_eq!(restored[0].geometry.y, Some(30.0));
        assert_eq!(restored[0].sidebar_width, 312.0);
        assert_eq!(restored[0].tabs[0].id, "saved-tab");
        assert_eq!(restored[0].tabs[0].title, "roundtrip");
        assert_eq!(
            restored[0].tabs[0].folder_state.view_mode,
            ViewMode::Gallery
        );
        assert_eq!(restored[0].tabs[0].folder_state.sort.key, SortKey::Kind);
        assert_eq!(
            restored[0].tabs[0].folder_state.sort.direction,
            SortDirection::Desc
        );

        std::fs::remove_file(database_path).ok();
    }
}
