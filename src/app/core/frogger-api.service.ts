import { Injectable } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";

import type { AppBootstrap, AppSettings, DirectoryListRequest, DirectoryListing, FolderViewState, SidebarState, SortState, ThumbnailDescriptor, WindowState } from "./frogger-api.types";

@Injectable({ providedIn: "root" })
export class FroggerApiService {
  bootstrap(): Promise<AppBootstrap> {
    return invoke<AppBootstrap>("bootstrap_app");
  }

  saveSessionState(windows: WindowState[]): Promise<void> {
    return invoke<void>("save_session_state", { windows });
  }

  createFileManagerWindow(path: string | null = null): Promise<WindowState> {
    return invoke<WindowState>("create_file_manager_window", { path });
  }

  listDirectory(
    path: string,
    sort: SortState,
    foldersFirst: boolean,
    hiddenFilesVisible: boolean,
    fileExtensionsVisible: boolean,
    cursor: string | null = null,
    limit: number | null = null,
  ): Promise<DirectoryListing> {
    const request: DirectoryListRequest = {
      path,
      sort,
      foldersFirst,
      hiddenFilesVisible,
      fileExtensionsVisible,
      cursor,
      limit,
    };

    return invoke<DirectoryListing>("list_directory", { request });
  }

  openFileWithDefaultApp(path: string): Promise<SidebarState> {
    return invoke<SidebarState>("open_file_with_default_app", { path });
  }

  getThumbnail(path: string): Promise<ThumbnailDescriptor | null> {
    return invoke<ThumbnailDescriptor | null>("get_thumbnail", { path });
  }

  cleanupThumbnailCache(): Promise<number> {
    return invoke<number>("cleanup_thumbnail_cache");
  }

  getSidebarState(): Promise<SidebarState> {
    return invoke<SidebarState>("get_sidebar_state");
  }

  setBrowserDisplaySetting(key: string, value: string): Promise<AppSettings> {
    return invoke<AppSettings>("set_browser_display_setting", { key, value });
  }

  getFolderViewState(path: string): Promise<FolderViewState> {
    return invoke<FolderViewState>("get_folder_view_state", { path });
  }

  saveFolderViewState(path: string, state: FolderViewState): Promise<void> {
    return invoke<void>("save_folder_view_state", { path, state });
  }

  pinSidebarFolder(path: string, label: string | null = null): Promise<SidebarState> {
    return invoke<SidebarState>("pin_sidebar_folder", { path, label });
  }

  unpinSidebarFolder(path: string): Promise<SidebarState> {
    return invoke<SidebarState>("unpin_sidebar_folder", { path });
  }

  setSidebarSectionVisibility(sectionId: string, visible: boolean): Promise<SidebarState> {
    return invoke<SidebarState>("set_sidebar_section_visibility", { sectionId, visible });
  }

  recordRecentItem(path: string): Promise<SidebarState> {
    return invoke<SidebarState>("record_recent_item", { path });
  }
}
