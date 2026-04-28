# PRD: Frogger Phase 1 — Finder-Style Desktop File Manager Initial State

## Problem Statement

Users need a faster, simpler, user-centric desktop file finder and folder manager that opens directly into a familiar directory layout instead of a branded dashboard or complicated onboarding flow. The first impression should feel like a high-performance Finder-style tool: familiar enough to use immediately, but architected for global search, indexing, future semantic search, and cross-platform support.

The current application is a fresh Tauri v2 + Angular starter. Phase 1 must replace the starter experience with Frogger’s initial file-manager shell, native-feeling browsing interactions, persistent session restoration, metadata indexing, basic file operations, search, previews, and settings.

## Solution

Build Frogger as a cross-platform Tauri v2 + Angular desktop app with a macOS Finder-inspired UI everywhere. On launch, Frogger restores the previous session: all windows, all tabs, active tab, last directories, selected items, view mode, sort state, sidebar width, window size/position, and other view preferences. On first-ever launch, Frogger opens one centered window with one Home tab, List view, Name ascending sort, hidden files hidden, and folders-first sorting enabled.

On first launch, Frogger requests broad filesystem access immediately. If permission is denied, the app shows an empty state explaining that file access is required and provides instructions to enable it in System Settings. After permission approval, the app shows the file-manager UI immediately, loads the visible directory, and runs a fast metadata indexing pass in parallel. While the initial no-index metadata build runs, search is disabled, the placeholder says “Indexing…”, and the bottom-right status area shows a minimal spinner plus “Indexing”. Once metadata indexing is available, search is enabled with the placeholder “Search”. Future background reconciliation can continue without disabling search.

The UI should mimic the supplied Finder screenshot in structure: macOS-style custom chrome, left sidebar, full top toolbar, main file area, and bottom path/status bar. Frogger should feel like a tool, not a brand. The app name should appear only where the app/menu system requires it.

## User Stories

1. As a desktop user, I want Frogger to reopen where I left off, so that I can resume work without navigating again.
2. As a first-time user, I want Frogger to open to my Home directory, so that the first state is predictable.
3. As a user, I want all previous windows restored, so that my workspace survives app restarts.
4. As a user, I want all previous tabs restored, so that multiple working directories remain available.
5. As a user, I want the last active tab focused on launch, so that the app resumes my exact context.
6. As a user, I want unavailable restored tabs dropped, so that I do not start in broken locations.
7. As a user, I want Frogger to open Home if no valid restored tabs remain, so that the app always has a usable fallback.
8. As a user, I want the previous selected item restored if it still exists, so that I can continue from my last selection.
9. As a user, I want the previous scroll position restored, so that large folders do not reset my place.
10. As a user, I want view mode restored, so that my preferred way of browsing is preserved.
11. As a user, I want sort order restored per folder, so that each folder behaves the way I configured it.
12. As a user, I want the window size and position restored, so that the app feels stable across sessions.
13. As a user, I want sidebar width restored, so that my layout remains comfortable.
14. As a user, I want active search cleared on normal launch, so that old searches do not make files appear missing.
15. As a first-time user, I want the app to request filesystem access immediately, so that I can use the file manager without repeated prompts.
16. As a user who denies permission, I want a clear empty state with instructions, so that I know how to recover.
17. As a user, I want the initial window to be large and centered, so that I can browse comfortably right away.
18. As a user, I want a Finder-like sidebar, so that the app feels familiar.
19. As a user, I want Recents first in the sidebar, so that recently opened items are easy to reach.
20. As a user, I want Recents to mean items opened through Frogger, so that the list is reliable and privacy-contained.
21. As a user, I want Recents to behave like a virtual folder, so that I can browse it using the current view mode.
22. As a user, I want Recents sorted by recently opened descending, so that the most relevant items appear first.
23. As a user, I want Favorites and Locations in the sidebar, so that common folders and drives are accessible.
24. As a user, I want Tags hidden until real tag support exists, so that the UI does not promise fake functionality.
25. As a user, I want mounted drives and detected cloud folders in Locations, so that common storage locations are visible.
26. As a cloud-folder user, I want cloud folder metadata indexed without forced downloads, so that search works without consuming bandwidth/storage.
27. As a cloud-folder user, I do not want previews or thumbnails to trigger downloads, so that cloud-only files remain cloud-only unless I choose otherwise.
28. As a user, I want to pin and unpin sidebar folders, so that my important folders are easy to access.
29. As a user, I want to hide or show sidebar sections, so that I can simplify the sidebar.
30. As a user, I want the sidebar to be collapsible, so that I can maximize file-list space.
31. As a user, I want the sidebar to be resizable, so that long folder names fit.
32. As a user, I want List, Grid/Icon, Column, and Gallery/Preview views, so that I can browse files in different contexts.
33. As a user, I want List view to be fully functional, so that the core file-manager workflow is reliable.
34. As a user, I want Grid, Column, and Gallery views to be usable in Phase 1, so that view switching is real even if advanced parity comes later.
35. As a user, I want easy toolbar view switching, so that I can change views quickly.
36. As a user, I want search results to always use List view, so that results are easy to scan.
37. As a user, I want normal List view columns to be Name, Date Modified, Size, and Kind, so that core metadata is visible.
38. As a search user, I want search result columns to be Name, Path, Date Modified, Size, and Kind, so that I understand where matches live.
39. As a user, I want list column visibility to be customizable globally, so that I can simplify or enrich the table.
40. As a user, I want column resizing, so that long names and paths are readable.
41. As a user, I want folders sorted before files by default, so that navigation targets appear first.
42. As a user, I want folders-first to be configurable, so that I can choose pure sort behavior.
43. As a user, I want hidden files hidden by default, so that normal browsing is clean.
44. As a user, I want hidden-file visibility remembered, so that developer workflows are supported.
45. As a user, I want file extensions hidden by default, so that the interface is Finder-like and simple.
46. As a user, I want file-extension visibility configurable, so that power-user workflows are supported.
47. As a user, I want file sizes shown for files, so that I can understand storage use.
48. As a user, I want folder sizes blank or dashed by default, so that browsing remains fast.
49. As a user, I want Kind labels based on practical app-defined categories initially, so that file types are understandable.
50. As a user, I want free icon-library file-type icons in Phase 1, so that files have visual affordances before custom icons exist.
51. As a user, I want image thumbnails cached locally, so that visual browsing gets faster over time.
52. As a user, I want thumbnails cached outside the main database with metadata for invalidation, so that cache performance and cleanup are manageable.
53. As a user, I want skeleton placeholders while directories load, so that loading feels intentional rather than broken.
54. As a user with huge folders, I want virtualized rendering, so that folders with tens of thousands of files remain responsive.
55. As a user, I want directory entries to appear with name, kind, size, and modified date, so that visible rows feel complete.
56. As a user, I want folders to open in the same window, so that navigation is predictable.
57. As a user, I want files to open with the default system app, so that Frogger works with my existing apps.
58. As a user, I want Back and Forward buttons visible and disabled until history exists, so that navigation is stable and familiar.
59. As a user, I want the toolbar title to show only the current folder name, so that the toolbar stays clean.
60. As a user, I want the bottom path bar visible by default, so that I always know where I am.
61. As a user, I want the bottom path bar hideable, so that I can reclaim vertical space.
62. As a user, I want bottom breadcrumbs to be clickable, so that I can navigate up the path quickly.
63. As a user, I want the bottom-right status to show item count or selected count, so that I understand the current folder/selection.
64. As a user, I want indexing to appear as only a spinner and “Indexing”, so that background work is visible but not noisy.
65. As a user, I want file operations to take priority over indexing in the bottom-right status, so that user-initiated work is emphasized.
66. As a user, I want the status area to open an activity popover, so that I can inspect active operations and failures.
67. As a user, I want the activity popover to show active operations and recent failed operations, so that problems are recoverable without a noisy history log.
68. As a user, I want indexing shown minimally in the activity popover, so that I know search preparation is happening.
69. As a user, I want indexing failures shown non-blockingly with recovery, so that search problems do not crash the app.
70. As a user, I want search disabled only during the initial no-index build, so that background reconciliation does not interrupt work.
71. As a user, I want global fuzzy file/folder search, so that I can find things even with partial or imperfect queries.
72. As a user, I want search to replace the main file list with results, so that the interaction feels Finder-like.
73. As a user, I want clearing search, pressing Escape, or pressing Back to exit search, so that I can return to browsing naturally.
74. As a user, I want search to be debounced while typing, so that results feel live without the app thrashing.
75. As a user, I want folders first in search results, so that navigational matches appear before files.
76. As a user, I want search ranking to prioritize folders, exact matches, prefix matches, fuzzy score, and recent/modified boosts, so that likely targets rise to the top.
77. As a user, I want paths visible in search results, so that global matches have context without extra scope UI.
78. As a user, I want double-clicking a folder in search results to open it and exit search, so that search becomes navigation.
79. As a user, I want double-clicking a file in search results to open it while keeping results visible, so that I can inspect multiple matches.
80. As a user, I want Reveal in Folder available from search results, so that I can jump to an item’s containing folder.
81. As a user, I want basic tabs, so that I can keep multiple folders open.
82. As a user, I want native macOS titlebar tabs where supported, so that tab behavior feels system-like.
83. As a user, I want to open folders in new tabs, close tabs, switch tabs, and restore tabs, so that tabs are practical.
84. As a user, I want multiple file-manager windows, so that I can manage separate workspaces.
85. As a user, I want native keyboard shortcuts per platform, so that macOS and Windows muscle memory both work.
86. As a keyboard user, I want arrows, Enter, Delete/Backspace, copy/cut/paste, select all, search, Escape, and rename shortcuts, so that Frogger is fast without a mouse.
87. As a user, I want standard multi-select with click, Cmd/Ctrl-click, Shift-click, and drag marquee, so that batch operations are easy.
88. As a user, I want Spacebar preview for supported files, so that I can inspect files quickly.
89. As a user, I want unsupported Spacebar previews to show icon and metadata, so that the interaction never feels broken.
90. As a user, I want Gallery view to have a large preview area and horizontal strip, so that it matches Finder expectations.
91. As a user, I want robust previews for images, PDFs, videos, audio, text/code, and common document types where feasible, so that browsing is useful.
92. As a user, I want previews implemented with a hybrid approach, so that common types work in the web UI and unsupported docs fall back gracefully.
93. As a user, I want no preview pane by default, so that the first-open layout stays clean.
94. As a user, I want no dedicated preview toolbar button, so that toolbar clutter stays low.
95. As a user, I want a visible New Folder toolbar button, so that folder creation is easy.
96. As a user, I want a visible Trash/Delete toolbar button disabled until selection, so that destructive actions are discoverable but safe.
97. As a user, I want a visible Sort toolbar control, so that sorting works outside List view too.
98. As a user, I do not want Group controls until grouping exists, so that the UI remains honest.
99. As a user, I do not want Share or Tags buttons until those features are functional, so that the toolbar has no fake controls.
100. As a user, I want an inactive AI toolbar button with tooltip “AI search coming soon”, so that the future direction is visible without clutter.
101. As a user, I want basic file operations, so that Frogger manages files rather than only browsing them.
102. As a user, I want New Folder, Rename, Move to Trash, Copy/Paste, Cut/Paste/Move, drag/drop, and default app opening, so that core file workflows work.
103. As a user, I want Delete to move items to Trash/Recycle Bin only, so that destructive mistakes are recoverable.
104. As a user, I want confirmations only for dangerous or ambiguous operations, so that normal workflows stay fast.
105. As a user, I want quick successful operations to update silently, so that the app feels instant and not noisy.
106. As a user, I want visible errors for failed operations, so that I know when work did not complete.
107. As a user, I want a Finder-like context menu, so that right-click workflows are available.
108. As a user, I want Open, Open With, Rename, Move to Trash, Copy, Cut, Paste, New Folder, Get Info, and Copy Path in context menus, so that common actions are nearby.
109. As a user, I want Open With to use the OS chooser/dialog where possible, so that I can pick apps without Frogger maintaining an app database.
110. As a user, I want Get Info to show name, kind, full path, size, dates, icon, and permissions/read-only status, so that I can inspect file metadata.
111. As a user, I want Copy Path to copy the absolute filesystem path, so that I can paste it into terminals and tools.
112. As a user, I want full OS drag/drop both directions, so that Frogger works with the desktop and other apps.
113. As a user, I want drag/drop to follow platform move/copy conventions, so that behavior matches OS expectations.
114. As a user, I want undo for rename and move-to-trash where reliable, so that common mistakes can be reversed.
115. As a user, I want Go to Folder via keyboard/menu command, so that I can jump to paths directly.
116. As a user, I want invalid Go to Folder paths to show inline errors, so that I can correct them without losing the dialog.
117. As a user, I want Open in Terminal for the current or selected folder, so that developer workflows are fast.
118. As a user, I want Open in Terminal to use the system default terminal initially, so that setup is simple.
119. As a user, I do not need Reveal in Finder/File Explorer in Phase 1, so that scope stays focused.
120. As a user, I want a full functional Settings screen, so that visible preferences actually work.
121. As a user, I want Settings accessed from the app menu and shortcut, so that the main toolbar remains Finder-like.
122. As a user, I want Appearance, Shortcuts, Indexing, AI model prep, Previews, Privacy, Sidebar, and Restore/session settings to be functional if shown, so that settings are trustworthy.
123. As a user, I want Frogger to follow system appearance, so that it matches my OS light/dark mode.
124. As a dark-mode user, I want dark mode polished equally to light mode, so that the app does not feel unfinished.
125. As a user, I want a translucent/material Finder-like sidebar, so that the app matches the desired macOS aesthetic.
126. As a Windows user, I want the same macOS-inspired custom chrome, so that Frogger’s visual identity is consistent across platforms.
127. As a Windows user, I still want Windows-native shortcuts, so that the macOS-inspired look does not break platform muscle memory.
128. As a user, I want Frogger to feel like a tool and not a brand, so that the interface stays focused on files.
129. As a user, I want the app name only in the app/menu name, so that the main UI is not branded.
130. As a user, I want indexing to include user-interactable folders, so that search covers files I actually use.
131. As a user, I want internal system/app/program files excluded from indexing, so that search is not noisy.
132. As a developer, I want dependency/build/cache folders excluded from indexing by default, so that search stays fast and relevant.
133. As a developer, I still want excluded folders visible while browsing, so that directory views remain truthful.
134. As a user, I want filesystem watchers and startup reconciliation, so that the metadata index stays fresh across app sessions.
135. As a user, I want deleted paths pruned and changed/new paths updated, so that search results are accurate.
136. As a future semantic-search user, I want the architecture to support local embeddings later, so that AI search can be added without reworking the app.
137. As a privacy-conscious user, I want indexing and future embeddings to be local, so that file metadata/content does not leave my computer.
138. As a user, I want exact, prefix, substring, and fuzzy metadata search before semantic search arrives, so that Frogger is already useful in Phase 1.

## Implementation Decisions

- Build on the existing Tauri v2 + Angular foundation.
- Use Rust/Tauri backend services for filesystem access, metadata indexing, file operations, session persistence, thumbnails, and OS integrations.
- Use Angular for the Finder-like desktop UI, state-driven views, context menus, settings, search results, previews, and activity popovers.
- Use SQLite as the local persistence layer for app/session state, folder view state, settings, Recents, file metadata index, indexing state, and thumbnail metadata.
- Keep image thumbnail binary files in the app cache directory while storing invalidation metadata in SQLite.
- Replace the starter screen with a Finder-style main shell.
- Use custom macOS-inspired chrome everywhere, including Windows, while preserving native platform keyboard shortcuts.
- Avoid main-UI branding. Use “Frogger” only where system menus/app metadata require it.
- On first-ever launch, request broad filesystem access immediately.
- If permission is denied, show an empty state with file-access-required instructions and recovery guidance.
- Restore all windows, tabs, active tabs, directories, selected items, scroll positions, view modes, sort state, sidebar width, and window geometry from SQLite.
- Drop unavailable restored tabs and open a single Home tab if none remain.
- Do not restore active search/filter state on normal launch.
- First-ever launch fallback is one centered large window, one Home tab, List view, Name ascending, hidden files hidden, folders-first on.
- Use a Finder-like layout: left sidebar, top toolbar, main file area, bottom path/status bar.
- Sidebar starts with Recents first, then Favorites and Locations. Tags are hidden until functional.
- Locations include mounted drives and detected cloud folders.
- Cloud folders are indexed for metadata only and must not trigger content downloads for indexing, thumbnails, or previews.
- Sidebar supports collapse, resizing, pin/unpin folders, and hide/show sections.
- Toolbar includes Back/Forward, current folder title, view switcher, Sort, New Folder, Trash/Delete, inactive AI button, and Search.
- Toolbar excludes Share and Tags until those features are functional.
- Toolbar title shows only the current folder name.
- Bottom path bar is visible by default, hideable, and has clickable breadcrumbs.
- Bottom status normally shows item count or selected count.
- Bottom status shows spinner plus “Indexing” during indexing unless a user-initiated operation is active.
- User-initiated file operations take bottom-status priority over indexing.
- Bottom status opens an activity popover for active operations, recent failures, and minimal indexing status.
- Implement List, Grid/Icon, Column, and Gallery/Preview views.
- List view is the primary fully functional view in Phase 1.
- Grid, Column, and Gallery views are usable but can be less advanced than List view.
- Remember view mode and sort per folder.
- Search results always use List view.
- Normal List columns default to Name, Date Modified, Size, Kind.
- Search List columns are Name, Path, Date Modified, Size, Kind.
- Column visibility is global. Column resizing is supported.
- Folders-first sorting is a setting and defaults on.
- File extensions are hidden by default and controlled by a setting.
- Hidden files are hidden by default and controlled by a remembered setting.
- Folder sizes are blank/dashed by default.
- Kind labels start as app-defined categories, with possible OS-native refinement later.
- Use a free icon library for file-type icons until custom icons are available.
- Use virtualized rendering for large directories.
- Directory loading should render skeleton placeholders matching the active view.
- Backend directory listing can stream entries progressively, while rows display complete core metadata once visible.
- Implement immediate metadata indexing after permission approval. During initial no-index build, search is disabled with placeholder “Indexing…”.
- Initial indexing is metadata-only: filename, path, kind/type/category, size, dates, folder/file status, and other minimal fields needed for fast search.
- After the initial metadata index exists, search remains enabled during background reconciliation.
- Use filesystem watchers while the app is open and startup reconciliation after app relaunch.
- Index user-interactable locations and exclude system/app/program internals.
- Exclude dependency/build/cache folders from indexing by default, including common project dependency, VCS, build output, virtual environment, and package cache directories.
- Index exclusions affect search/indexing only, not normal directory browsing.
- Implement global fuzzy filename/folder search over metadata.
- Search replaces the main file list with results.
- Search exits via clearing the field, Escape, or Back, and search state participates in navigation history.
- Search uses 100–150ms debounced instant queries.
- Search ranking is folders first, exact match, prefix match, fuzzy score, then recently opened/modified boost.
- Search uses the Path column for global-result context instead of visible scope chips.
- Double-clicking a search-result folder opens it and exits search.
- Double-clicking a search-result file opens it with the default system app and keeps search results visible.
- Search results support Reveal in Folder via context/action.
- Implement basic tabs: open folder in new tab, close tab, switch tab, restore tabs.
- Prefer native macOS titlebar tabs where the framework supports them.
- Implement multiple file-manager windows with per-window tabs and restoration.
- Implement core keyboard navigation and platform-native shortcut mappings.
- Implement standard multi-select in list/grid/column contexts.
- Implement basic Spacebar preview with supported previews and fallback metadata/icon.
- Implement Gallery view as a Finder-like large preview area with horizontal thumbnail/file strip.
- Implement hybrid previews: Angular/web for images, video, audio, text/code, and PDFs; fallback icon/metadata for unsupported document types; OS-native preview APIs can come later.
- Do not show a preview pane/sidebar by default.
- Do not include a dedicated preview toolbar button.
- Implement New Folder, Rename, Move to Trash/Recycle Bin, Copy/Paste, Cut/Paste/Move, drag/drop, Open, Open With, Get Info, Copy Path, Go to Folder, and Open in Terminal.
- Delete means Move to Trash/Recycle Bin only. No permanent delete in Phase 1.
- Confirm only dangerous or ambiguous operations such as overwrite/merge conflicts, extension changes, or permission/system errors.
- Successful quick operations update the UI silently.
- Long operations show bottom-right status and activity popover details.
- Use OS chooser/dialog for Open With where possible.
- Get Info is an app-owned modal/panel with basic metadata.
- Copy Path copies absolute filesystem paths only.
- Full OS drag/drop works both into and out of Frogger.
- Drag/drop follows standard platform move/copy behavior and modifier overrides.
- Support undo for rename and move-to-trash where reliable.
- Implement Go to Folder through menu/shortcut with inline invalid-path errors.
- Implement Open in Terminal using the system default terminal in Phase 1.
- Do not implement Reveal in Finder/File Explorer in Phase 1 outside the search-result Reveal in Folder behavior already specified.
- Implement a full functional Settings screen accessed through app menu and shortcut, not a toolbar gear.
- Settings shown in Phase 1 must work. Sections include appearance/system theme behavior, shortcuts, indexing locations/exclusions, AI model/semantic prep, previews, privacy, sidebar, restore/session behavior, hidden files, file extensions, folders-first sorting, and path bar visibility.
- Follow system appearance for light/dark mode. Dark mode must be polished to light-mode quality.
- Include an inactive AI toolbar button with tooltip “AI search coming soon.” Semantic search/local embeddings are prepared architecturally but not active in initial metadata indexing.

## Testing Decisions

Good tests should verify external behavior through stable module interfaces rather than implementation details. Tests should assert user-visible outcomes, persisted state, filesystem side effects, search results, and error/recovery behavior.

Recommended test coverage:

- Session persistence module: restores windows, tabs, active tabs, folder state, selected items, window geometry, and handles unavailable folders/tabs.
- Settings module: persists and applies hidden files, file extensions, folders-first, path bar visibility, sidebar sections, indexing exclusions, restore behavior, and theme mode.
- Filesystem browsing module: lists directories, handles permission errors, excludes hidden files when configured, shows excluded indexed folders during browsing, and handles huge directories through streaming/virtualization contracts.
- Metadata indexing module: builds initial metadata index, runs startup reconciliation, prunes deleted paths, updates changed/new paths, skips excluded folders, indexes cloud metadata without content downloads, and reports indexing failures.
- Search module: disables during initial no-index state, enables after metadata index exists, performs debounced fuzzy queries, ranks folders/exact/prefix/fuzzy/recent matches correctly, and returns expected search result columns.
- File operation module: creates folders, renames, copies, cuts/moves, moves to trash/recycle bin, handles conflicts/extension-change confirmations, updates UI silently on success, and reports failures.
- Undo module: supports undo rename and undo move-to-trash where platform restoration is reliable.
- Drag/drop module: accepts external drops, exports files to OS/apps, performs internal move/copy behavior, and respects platform modifier conventions.
- Preview module: renders supported image/video/audio/text/PDF previews, falls back for unsupported files, avoids cloud-only downloads, and supports Spacebar preview behavior.
- Thumbnail cache module: creates, retrieves, invalidates, and cleans image thumbnail cache entries.
- Sidebar module: displays Recents, Favorites, Locations, pins/unpins folders, hides/shows sections, and persists width/collapsed state.
- Recents module: records app-opened files/folders and displays them sorted by recently opened descending.
- Activity module: prioritizes user operations over indexing, shows active operations, shows recent failures, and provides retry/recovery for indexing failures.
- Keyboard shortcut module: maps macOS and Windows shortcuts correctly while sharing UI behavior.
- Context menu module: exposes only functional Phase 1 actions and disables unavailable actions appropriately.
- Settings UI tests: verify every visible setting changes real behavior.

Prior art in the current codebase is minimal because the project is still the starter template. New tests should establish the testing patterns for Angular component behavior, Rust backend unit tests, and integration tests across Tauri command interfaces.

## Out of Scope

- Full semantic content search using local embeddings as an active Phase 1 feature.
- Full-computer content embedding within five seconds.
- Share functionality.
- Tag display/edit/filter functionality.
- Grouping in the Sort control.
- Permanent delete.
- Full file-operation undo for every operation.
- Drag/drop onto bottom breadcrumb folders.
- Full OS-native Quick Look / Windows Preview Handler integration.
- Custom final Frogger logo/icon asset work.
- Full Office document rendering if not feasible with the hybrid preview approach.
- Network share discovery beyond easily detected mounted locations.
- User-selectable terminal app setting.
- App-owned Open With app database.
- Main UI branding beyond required app/menu name.
- Reveal in Finder/File Explorer as a general Phase 1 feature, except search-result Reveal in Folder behavior.
- Advanced sidebar reorder/rename customization.
- Column reorder and full per-folder column customization.
- Tags section in sidebar.
- Fake or disabled Share/Tags controls.

## Further Notes

- The Phase 1 scope is intentionally larger than a simple directory loadout. It includes the file-manager shell, persistent workspace restoration, metadata indexing, global fuzzy search, basic file operations, previews, tabs, multiple windows, settings, and platform-aware shortcuts.
- The five-second indexing ambition should apply to the initial metadata index, not full semantic embeddings. Semantic embeddings should be architected for later local-only incremental indexing.
- The core product feel should be “Finder, but faster and calmer.” Familiarity is the first-open goal; speed and future AI search are the differentiators.
- Since the UI is intentionally macOS-inspired on all platforms, extra care is needed to preserve Windows shortcut behavior and file operation conventions.
- The app should avoid fake controls. If a feature is visible in Phase 1, it should be functional unless explicitly specified as inactive with tooltip-only behavior, such as the AI button.
