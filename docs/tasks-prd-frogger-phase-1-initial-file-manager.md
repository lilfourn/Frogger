# Task Plan: Frogger Phase 1 — Finder-Style Desktop File Manager Initial State

## Summary

Build Frogger Phase 1 by replacing the Tauri v2 + Angular starter with a Finder-style cross-platform desktop file manager. The implementation must ship a native-feeling file browsing shell, session/window/tab restoration, SQLite persistence, metadata indexing, fuzzy global search, previews, core file operations, settings, activity reporting, and platform-aware shortcuts.

The project is currently a minimal Angular 20 + Tauri v2 starter:

- Frontend: `src/app/*`, `src/styles.css`, Angular standalone app structure.
- Backend: `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`, minimal Tauri commands.
- Config: `package.json`, `angular.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.

## Assumptions

- SQLite will be embedded in the Rust backend using a Rust SQLite crate or Tauri SQL integration, with migrations owned by `src-tauri`.
- Broad filesystem access differs by platform. macOS recovery guidance may need Full Disk Access instructions; Windows and Linux should use capability checks and graceful errors.
- Native macOS titlebar tabs are preferred only where Tauri and the OS expose reliable support. A custom tab strip must still exist for cross-platform consistency.
- Phase 1 can use app-defined Kind labels and a free icon set rather than OS-native icons.
- Cloud-provider detection can start with common local provider folders and platform metadata flags. It must avoid reads that force hydration/downloads.
- Gallery, Grid/Icon, and Column views must be usable, but List view receives the deepest functionality and test coverage in Phase 1.
- Semantic search and embeddings are architectural placeholders only. The inactive AI toolbar button is the only visible AI feature.

## Requirement Inventory

| ID | Priority | Type | Requirement | Source/Notes |
|---|---:|---|---|---|
| REQ-001 | P0 | frontend | Replace starter UI with a Finder-like unbranded shell: custom chrome, sidebar, toolbar, main file area, bottom path/status bar. | Problem, Solution, stories 18, 59, 60, 125-129 |
| REQ-002 | P0 | platform | First-ever launch opens one large centered Home window with List view, Name ascending sort, folders-first on, hidden files hidden, extensions hidden. | Solution, stories 2, 17, 41, 43, 45 |
| REQ-003 | P0 | platform | Request broad filesystem access immediately and show a recovery empty state if access is denied. | Stories 15-16 |
| REQ-004 | P0 | data | Persist app/session state in SQLite, including windows, tabs, active tab, directories, selection, scroll, view mode, sort, sidebar width, window geometry, settings, recents, metadata, index state, and thumbnails metadata. | Implementation decisions |
| REQ-005 | P0 | platform | Restore all valid windows/tabs/session state on launch, drop unavailable tabs, fall back to Home when needed, and never restore active search on normal launch. | Stories 1, 3-14 |
| REQ-006 | P0 | frontend | Implement sidebar with Recents first, Favorites, Locations, mounted drives, detected cloud folders, pin/unpin, hide/show sections, collapse, resize, and no Tags section. | Stories 19-31 |
| REQ-007 | P0 | data | Recents are only items opened through Frogger, behave as a virtual folder, and sort by recently opened descending. | Stories 20-22 |
| REQ-008 | P0 | backend | List directories with core metadata, hidden-file filtering, extension display setting, folder sizes blank/dashed, app-defined Kind labels, file-type icons, skeleton loading, and huge-folder performance. | Stories 32-35, 47-55 |
| REQ-009 | P0 | frontend | Implement List, Grid/Icon, Column, and Gallery/Preview views; List is fully functional; search results always use List view. | Stories 32-36, 90 |
| REQ-010 | P1 | frontend | Support list/search columns, global column visibility, and column resizing. | Stories 37-40 |
| REQ-011 | P0 | frontend | Implement navigation: same-window folder opening, default-app file opening, back/forward, breadcrumbs, bottom path bar, and item/selection status. | Stories 56-63 |
| REQ-012 | P0 | backend | Implement metadata-only initial indexing, local-only architecture, SQLite index, initial search-disable state, and future semantic-search readiness. | Stories 64, 70, 130-138 |
| REQ-013 | P0 | backend | Implement indexing exclusions, cloud metadata safety, watchers, startup reconciliation, pruning deleted paths, and update changed/new paths. | Stories 26-27, 130-135 |
| REQ-014 | P0 | frontend | Show indexing minimally in status and activity popover, prioritize user file operations over indexing, and show non-blocking indexing failures with recovery. | Stories 64-69 |
| REQ-015 | P0 | backend | Implement global fuzzy file/folder metadata search with debounce support, folders-first ranking, exact/prefix/fuzzy/recent/modified boosts, and path context. | Stories 71-77, 138 |
| REQ-016 | P0 | frontend | Search replaces main list, exits through clear/Escape/Back, participates in navigation, opens folders by exiting search, opens files while keeping results, and supports Reveal in Folder. | Stories 72-80 |
| REQ-017 | P0 | platform | Implement tabs and multiple windows with folder-in-new-tab, close/switch/restore tabs, per-window tabs, and native macOS tabs where reliable. | Stories 81-84 |
| REQ-018 | P0 | frontend | Implement native keyboard shortcuts per platform, keyboard navigation, standard multi-select, and drag marquee. | Stories 85-87 |
| REQ-019 | P0 | frontend | Implement Spacebar preview, fallback icon/metadata, Gallery preview area, hybrid web previews for common types, no default preview pane, and no preview toolbar button. | Stories 88-94 |
| REQ-020 | P1 | data | Cache image thumbnails outside the main database and store invalidation metadata in SQLite. | Stories 51-52 |
| REQ-021 | P0 | backend | Implement core file operations: New Folder, Rename, Trash/Recycle Bin, Copy/Paste, Cut/Paste/Move, default open, silent quick success, visible failures, and confirmations for dangerous/ambiguous cases. | Stories 95-106 |
| REQ-022 | P0 | frontend | Implement Finder-like context menus: Open, Open With, Rename, Move to Trash, Copy, Cut, Paste, New Folder, Get Info, Copy Path. | Stories 107-111 |
| REQ-023 | P0 | platform | Implement OS drag/drop both directions with platform move/copy conventions and modifier overrides. | Stories 112-113 |
| REQ-024 | P1 | backend | Support undo for rename and move-to-trash where reliable. | Story 114 |
| REQ-025 | P1 | platform | Implement Go to Folder with inline invalid-path errors and Open in Terminal using system default terminal. | Stories 115-118 |
| REQ-026 | P0 | frontend | Toolbar must include Back/Forward, folder title, view switcher, Sort, New Folder, Trash/Delete, inactive AI button with tooltip, and Search; must exclude Share, Tags, Grouping, and preview button. | Stories 95-100 |
| REQ-027 | P0 | frontend | Implement a functional Settings screen from app menu/shortcut for appearance, shortcuts, indexing, AI prep, previews, privacy, sidebar, restore/session, hidden files, extensions, folders-first, and path bar. | Stories 120-123 |
| REQ-028 | P0 | design | Follow system appearance, polish dark mode, keep macOS-inspired custom chrome on all platforms, and preserve Windows-native shortcut behavior. | Stories 123-127 |
| REQ-029 | P0 | test | Add Angular, Rust, and command-interface tests for persistence, settings, browsing, indexing, search, file operations, undo, drag/drop, previews, thumbnails, sidebar, recents, activity, shortcuts, context menus, and settings UI. | Testing decisions |
| REQ-030 | P2 | scope | Keep out-of-scope items excluded: active semantic search, Share, Tags, grouping, permanent delete, general Reveal in Finder/File Explorer, advanced sidebar customization, full OS Quick Look/Preview Handler, terminal app selector, Open With database, and main UI branding. | Out of Scope |

## Phases

1. **Foundation and contracts** — Add dependencies, database schema, backend command boundaries, first-launch permission flow.
2. **Finder shell and browsing** — Build the visible shell, session restore, sidebar, tabs, navigation, directory listing, and List view.
3. **Indexing, search, alternate views, thumbnails** — Add metadata indexing, reconciliation, fuzzy search, Grid/Column/Gallery, and thumbnail cache.
4. **File operations and interaction depth** — Add operations, context menus, keyboard shortcuts, drag/drop, activity popover, undo, Get Info, Go to Folder, Terminal.
5. **Previews, settings, polish, and hardening** — Add hybrid previews, functional settings, theme/chrome polish, full tests, and release checks.

## Tasks

## Task 1: Establish project identity, dependencies, and test scaffolding
Priority: P0
Phase: Foundation and contracts
Depends on: none
Requirements: REQ-001, REQ-004, REQ-028, REQ-029, REQ-030

### Goal
Prepare the starter app for Frogger implementation with correct app metadata, required libraries, and repeatable test commands.

### Implementation Notes
- Update `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` from starter naming to Frogger where system metadata requires it.
- Add frontend packages for virtual scrolling, icons, keyboard handling, and component testing if selected by the team.
- Add Rust crates for SQLite, filesystem walking, ignore/exclusion matching, filesystem watching, fuzzy search, trash/recycle-bin behavior, path utilities, async work, and test utilities.
- Add scripts for Angular tests, Rust tests, lint/build checks, and Tauri development.
- Keep main UI branding out of Angular templates and styles.

### Acceptance Criteria
- [ ] App metadata/menu name is Frogger while the main shell does not show brand-first dashboard content.
- [ ] Required frontend and backend dependencies are declared and install/build cleanly.
- [ ] A documented test command exists for Angular, Rust, and full app build verification.
- [ ] Out-of-scope controls such as Share, Tags, Grouping, and permanent delete are not introduced.

### Verification
- [ ] Run `bun install` or the project package manager equivalent successfully.
- [ ] Run `bun run build` successfully.
- [ ] Run `cd src-tauri && cargo test` successfully.
- [ ] Inspect app config to confirm Frogger appears only in app/system metadata.

## Task 2: Implement SQLite persistence schema and migration layer
Priority: P0
Phase: Foundation and contracts
Depends on: Task 1
Requirements: REQ-004, REQ-005, REQ-006, REQ-007, REQ-012, REQ-020, REQ-027, REQ-029

### Goal
Create the durable data foundation for session restoration, settings, recents, metadata indexing, thumbnail metadata, and activity state.

### Implementation Notes
- Add a Rust persistence module under `src-tauri/src/` for opening the app-local SQLite database.
- Create migrations for windows, tabs, folder view state, selected items, scroll positions, settings, sidebar sections, favorites, recents, metadata index rows, index checkpoints, thumbnail metadata, and recent failed operations.
- Store window geometry and sidebar width per window.
- Store sort and view state per folder when required and global settings when required.
- Include migration versioning and deterministic migration tests.

### Acceptance Criteria
- [ ] Fresh launch creates the database in the app data directory.
- [ ] Repeated launches do not corrupt or duplicate migration state.
- [ ] Database tables cover every persisted Phase 1 state category.
- [ ] Migrations are idempotent and versioned.

### Verification
- [ ] Add and pass Rust tests for fresh migration, repeated migration, and sample insert/read for settings, tabs, and metadata rows.
- [ ] Manually inspect the created SQLite schema during a dev run.
- [ ] Run a clean-profile launch and confirm no persistence errors appear.

## Task 3: Define Tauri command, event, and frontend state contracts
Priority: P0
Phase: Foundation and contracts
Depends on: Task 2
Requirements: REQ-004, REQ-008, REQ-011, REQ-012, REQ-015, REQ-017, REQ-021, REQ-027, REQ-029

### Goal
Create stable interfaces between Angular and Rust before implementing each feature deeply.

### Implementation Notes
- Define serializable Rust DTOs for filesystem entries, tabs, windows, folder state, settings, search results, operations, previews, activities, and errors.
- Register Tauri commands for app bootstrap, directory listing, navigation validation, search, settings, session persistence, file operations, previews, thumbnails, context actions, and activity queries.
- Define event names for directory listing progress, indexing progress, file operation progress, watcher updates, settings changes, and activity failures.
- Add Angular services in `src/app/` for invoking commands and subscribing to events.
- Normalize error shape so permission, missing path, conflict, and operation failures render consistently.

### Acceptance Criteria
- [ ] Angular can call a bootstrap command and receive typed initial state.
- [ ] Rust command errors serialize into a stable frontend error model.
- [ ] Events can be subscribed to and unsubscribed from without leaks.
- [ ] DTOs include enough data for List view, search results, previews, and status UI.

### Verification
- [ ] Add Rust unit tests for DTO serialization of representative payloads.
- [ ] Add Angular service tests with mocked Tauri invoke/listen APIs.
- [ ] Run the app and confirm bootstrap data renders in a temporary diagnostic-free shell state.

## Task 4: Implement first-launch filesystem access and denied-access recovery
Priority: P0
Phase: Foundation and contracts
Depends on: Task 3
Requirements: REQ-002, REQ-003, REQ-008, REQ-030

### Goal
Ensure first-time users are immediately put into a usable Home browsing flow or a clear recovery state when filesystem access is unavailable.

### Implementation Notes
- On first-ever launch, resolve the user Home directory and validate read access.
- Request or trigger the broadest practical filesystem access path per platform within Tauri constraints.
- Add a full-window empty state explaining that file access is required and giving System Settings recovery guidance where applicable.
- Provide a retry action that rechecks access and enters the file manager without restarting.
- Avoid onboarding screens, branded dashboards, or multi-step setup.

### Acceptance Criteria
- [ ] First-ever permitted launch opens Home in a large centered window with expected defaults.
- [ ] Denied or unavailable access shows a clear empty state with recovery instructions.
- [ ] Retry after permission approval loads the file manager UI immediately.
- [ ] No search, fake sidebar content, or broken directory list appears while access is denied.

### Verification
- [ ] Add Rust tests for Home path resolution and access-check error classification.
- [ ] Add Angular component tests for denied-access empty state and retry behavior.
- [ ] Manually test a clean app profile with allowed and denied access paths.

## USER-TEST Checkpoint 1: Launch and access foundation

### What to Verify
- Fresh app launch no longer shows the starter screen.
- First-ever launch lands in Home when filesystem access is available.
- Denied access produces a clear recovery empty state.
- Build and test commands are stable for future work.

### Happy Path
- Delete local app data, run the app, approve filesystem access, and confirm a centered Home window appears.

### Edge/Error Cases
- Deny or simulate denied Home access and confirm the recovery state is understandable and retry works.
- Relaunch repeatedly and confirm migrations do not rerun destructively.

### Regression Checks
- Main UI does not show Frogger branding beyond system metadata.
- No out-of-scope Share, Tags, Grouping, or permanent delete controls appear.

### Stop/Go Criteria
- Continue only if a clean profile can enter Home or the access recovery state reliably.

## Task 5: Implement session restoration for windows, tabs, and folder state
Priority: P0
Phase: Finder shell and browsing
Depends on: Task 4
Requirements: REQ-002, REQ-004, REQ-005, REQ-017, REQ-029

### Goal
Restore the user's previous workspace exactly enough to feel stable across restarts.

### Implementation Notes
- Persist and restore all windows, tabs, active tabs, directories, selected items, scroll positions, view modes, sort state, sidebar width, and window geometry.
- Validate restored tab paths before using them.
- Drop unavailable restored tabs and use one Home tab when no valid tab remains.
- Clear any active search/filter state on normal launch.
- Persist session changes on meaningful state transitions and on close.

### Acceptance Criteria
- [ ] Relaunch restores all valid windows and tabs.
- [ ] Active tab, selected item, scroll position, view mode, sort, sidebar width, and geometry are restored.
- [ ] Unavailable tabs are dropped without blocking launch.
- [ ] Home opens when all restored tabs are invalid.
- [ ] Active search is not restored.

### Verification
- [ ] Add Rust tests for restore fallback and invalid-tab pruning.
- [ ] Add Angular state tests for applying restored active tab and folder state.
- [ ] Manually open multiple tabs/windows, change state, quit, relaunch, and compare restored state.

## Task 6: Build multi-window and tab management foundation
Priority: P0
Phase: Finder shell and browsing
Depends on: Task 5
Requirements: REQ-005, REQ-017, REQ-028, REQ-029

### Goal
Support practical file-manager workspaces with multiple windows and tabs.

### Implementation Notes
- Implement open folder in new tab, close tab, switch tab, and restore tabs through Angular state and persisted backend state.
- Add window creation commands and per-window session identifiers.
- Prefer native macOS titlebar tabs only if reliable; otherwise keep custom tabs while preserving custom chrome.
- Ensure each window owns independent navigation history, active search state, sidebar state, and view state.

### Acceptance Criteria
- [ ] Users can open folders in new tabs, close tabs, and switch tabs.
- [ ] Users can open multiple file-manager windows.
- [ ] Each window restores its own tabs after restart.
- [ ] Closing a tab never leaves a window without a valid fallback tab.

### Verification
- [ ] Add frontend state tests for tab open/close/switch behavior.
- [ ] Add backend tests for persisted per-window tab groups.
- [ ] Manually verify two windows with distinct active tabs restore correctly.

## Task 7: Implement Finder-style shell layout, toolbar, and bottom bars
Priority: P0
Phase: Finder shell and browsing
Depends on: Task 6
Requirements: REQ-001, REQ-011, REQ-026, REQ-028, REQ-030

### Goal
Create the core visual and interaction frame that replaces the starter experience.

### Implementation Notes
- Replace `app.component.html`, `app.component.ts`, and styles with a shell composed of custom chrome, sidebar, toolbar, main content region, path bar, and status area.
- Toolbar includes Back/Forward, current folder title only, view switcher, Sort, New Folder, Trash/Delete, inactive AI button with tooltip “AI search coming soon”, and Search.
- Toolbar excludes Share, Tags, Group, preview button, and settings gear.
- Bottom path bar is visible by default, hideable later through settings, and supports clickable breadcrumbs.
- Bottom-right status shows item count or selected count when idle.

### Acceptance Criteria
- [ ] The UI structurally matches a Finder-like file manager rather than an Angular starter page.
- [ ] Toolbar controls match Phase 1 scope exactly.
- [ ] Back/Forward are visible and disabled until history exists.
- [ ] Folder title displays only the current folder name.
- [ ] Bottom path/status bar is visible and useful.

### Verification
- [ ] Add Angular component tests for toolbar control presence and disabled states.
- [ ] Manually compare first-open structure against the PRD screenshot intent.
- [ ] Verify settings are not exposed as a toolbar gear.

## Task 8: Implement sidebar sections, Locations, Favorites, and Recents
Priority: P0
Phase: Finder shell and browsing
Depends on: Task 7
Requirements: REQ-006, REQ-007, REQ-013, REQ-027, REQ-029, REQ-030

### Goal
Provide a familiar, persistent Finder-style sidebar with real behavior and no fake tag support.

### Implementation Notes
- Sidebar sections are Recents, Favorites, and Locations in that order.
- Locations include mounted drives and detected cloud folders.
- Recents includes only files/folders opened through Frogger and behaves like a virtual folder in the current view mode.
- Implement pin/unpin for sidebar folders.
- Implement hide/show section settings, sidebar collapse, and sidebar resizing with persisted width.
- Keep Tags hidden until real tag support exists.

### Acceptance Criteria
- [ ] Recents appears first and only contains app-opened items.
- [ ] Favorites can pin and unpin folders.
- [ ] Locations includes mounted drives and detected common cloud folders when present.
- [ ] Sidebar sections can be hidden/shown and the choices persist.
- [ ] Sidebar collapse and width persist across restarts.

### Verification
- [ ] Add backend tests for recents insertion and descending sort.
- [ ] Add Angular tests for sidebar section rendering and persistence calls.
- [ ] Manually open files/folders and confirm Recents updates without including unrelated OS history.

## Task 9: Implement directory listing backend with streaming-ready metadata
Priority: P0
Phase: Finder shell and browsing
Depends on: Task 3
Requirements: REQ-008, REQ-011, REQ-013, REQ-021, REQ-029

### Goal
List real filesystem directories quickly and safely with all metadata needed for the browser UI.

### Implementation Notes
- Implement Rust directory listing command that returns entries with name, path, file/folder status, size, modified date, kind/category, hidden status, extension, read-only status, and cloud/hydration hints where available.
- Support progressive or chunked results for huge directories through events or paged commands.
- Apply hidden-file filtering for browsing based on user setting.
- Do not apply indexing exclusions to normal directory browsing.
- Return permission and missing-path errors with recoverable error codes.

### Acceptance Criteria
- [ ] Home and regular folders list real entries with complete visible metadata.
- [ ] Hidden files are excluded when the setting is off.
- [ ] Excluded dependency/build/cache folders remain visible in browsing.
- [ ] Large directories can be listed progressively or through a virtualization-friendly contract.
- [ ] Permission and missing-path errors show recoverable UI states.

### Verification
- [ ] Add Rust tests using temporary directories for files, folders, hidden files, permissions where practical, and excluded folder names.
- [ ] Add command integration tests for error serialization.
- [ ] Manually browse Home, an empty folder, and a large folder.

## Task 10: Build the fully functional virtualized List view
Priority: P0
Phase: Finder shell and browsing
Depends on: Task 9
Requirements: REQ-008, REQ-009, REQ-010, REQ-011, REQ-018, REQ-029

### Goal
Deliver the primary Phase 1 browsing experience with high performance and reliable selection.

### Implementation Notes
- Build a List view component with virtualized rows and skeleton placeholders while directories load.
- Default columns are Name, Date Modified, Size, and Kind.
- Display file sizes for files and blank/dash folder sizes by default.
- Use free icon-library icons for file-type affordances.
- Implement row selection, double-click folder navigation, double-click file open, and status count updates.
- Restore selected item and scroll offset when possible.

### Acceptance Criteria
- [ ] Directories render as a complete List view with expected columns.
- [ ] Tens of thousands of entries remain responsive through virtualization.
- [ ] Skeleton placeholders appear while loading.
- [ ] Double-clicking folders opens in the same window.
- [ ] Double-clicking files opens with the default system app and records Recents.
- [ ] Item count and selected count update in the bottom-right status.

### Verification
- [ ] Add Angular component tests for rows, columns, selection, double-click actions, and skeleton state.
- [ ] Add manual performance check with a generated large directory.
- [ ] Verify selection and scroll restoration after navigating away and back.

## Task 11: Implement sorting, column preferences, and browsing display settings
Priority: P0
Phase: Finder shell and browsing
Depends on: Task 10
Requirements: REQ-004, REQ-008, REQ-010, REQ-027, REQ-029

### Goal
Make folder presentation configurable and persistent while keeping defaults Finder-like.

### Implementation Notes
- Implement Name ascending default sort and folders-first default behavior.
- Persist sort state per folder.
- Add global settings for folders-first, hidden files, file extensions, and column visibility.
- Support column resizing in normal List view and search results.
- Hide file extensions by default while retaining full names for operations and copy path.

### Acceptance Criteria
- [ ] Folders sort before files by default.
- [ ] Users can change sort and the folder remembers it.
- [ ] Hidden-file visibility persists.
- [ ] File-extension visibility persists and changes display without corrupting operations.
- [ ] Column visibility and widths persist globally.

### Verification
- [ ] Add Angular tests for sort order, folders-first toggle, hidden-file toggle, extension display, and column resizing.
- [ ] Add persistence tests for folder sort and global column settings.
- [ ] Manually relaunch and confirm changed display settings remain applied.

## USER-TEST Checkpoint 2: Finder shell, restore, sidebar, and List browsing

### What to Verify
- The app feels like a Finder-style file manager on launch.
- Windows/tabs/sidebar/list state restore after quitting.
- Home browsing, folder navigation, default-app file opening, and Recents work.
- List view handles large folders without freezing.

### Happy Path
- Open Home, navigate into folders, open a file, pin a folder, resize the sidebar, open a new tab, quit, relaunch, and verify state restoration.

### Edge/Error Cases
- Delete or rename a restored folder externally and relaunch.
- Browse a folder with hidden files while the hidden-files setting is off and on.
- Browse a generated large folder.

### Regression Checks
- Tags section remains hidden.
- Search is not restored after relaunch.
- Toolbar still excludes Share, Tags, Grouping, and preview controls.

### Stop/Go Criteria
- Continue only if List browsing and session restoration are reliable enough to use as the base for search and operations.

## Task 12: Implement Grid/Icon, Column, and Gallery view modes
Priority: P0
Phase: Indexing, search, alternate views, thumbnails
Depends on: Task 11
Requirements: REQ-009, REQ-011, REQ-018, REQ-019, REQ-029

### Goal
Make view switching real across all Phase 1 view modes while keeping List view primary.

### Implementation Notes
- Add shared view-state model so List, Grid/Icon, Column, and Gallery consume the same directory entries and selection state.
- Grid/Icon view shows icons or thumbnails, names, selection, and double-click behavior.
- Column view supports hierarchical folder navigation across columns.
- Gallery view shows a large preview area and horizontal file strip.
- Persist view mode per folder.

### Acceptance Criteria
- [ ] Toolbar view switcher changes between List, Grid/Icon, Column, and Gallery.
- [ ] All views can browse folders and select/open files.
- [ ] Gallery has a large preview area and horizontal strip.
- [ ] View mode persists per folder.
- [ ] Search results continue to force List view.

### Verification
- [ ] Add Angular tests for view switching and per-view basic navigation.
- [ ] Manually exercise selection and double-click behavior in all views.
- [ ] Relaunch and confirm folder view modes restore.

## Task 13: Implement thumbnail cache and image thumbnail generation
Priority: P1
Phase: Indexing, search, alternate views, thumbnails
Depends on: Task 12
Requirements: REQ-013, REQ-020, REQ-029

### Goal
Provide faster visual browsing with cacheable thumbnails that do not bloat the main database.

### Implementation Notes
- Store thumbnail image files in the app cache directory.
- Store source path, modified time, size, thumbnail path, dimensions, and invalidation data in SQLite.
- Generate thumbnails for local image files only when safe.
- Avoid thumbnail generation that hydrates cloud-only files.
- Add cleanup for orphaned and stale thumbnail entries.

### Acceptance Criteria
- [ ] Image thumbnails appear in Grid and Gallery where available.
- [ ] Reopening a folder uses cached thumbnails when source metadata is unchanged.
- [ ] Changed source files invalidate stale thumbnails.
- [ ] Cloud-only files do not download solely for thumbnails.
- [ ] Thumbnail binaries are outside the main SQLite database.

### Verification
- [ ] Add Rust tests for cache lookup, invalidation, and cleanup metadata behavior.
- [ ] Add manual checks that repeated folder opens reuse cached thumbnails.
- [ ] Test cloud-only or simulated cloud-placeholder files do not trigger thumbnail reads.

## Task 14: Implement initial metadata indexing and search availability state
Priority: P0
Phase: Indexing, search, alternate views, thumbnails
Depends on: Task 2, Task 9
Requirements: REQ-012, REQ-014, REQ-015, REQ-029

### Goal
Build the local metadata index needed for global search and expose correct initial indexing UI behavior.

### Implementation Notes
- Start a fast metadata-only indexing pass after filesystem permission approval.
- Index filename, path, kind/type/category, size, dates, folder/file status, recent/open boost fields, and minimal search fields.
- Store index state/checkpoints in SQLite.
- During the initial no-index build, disable search, set placeholder to “Indexing…”, and show bottom-right spinner plus “Indexing”.
- Once an index exists, enable search with placeholder “Search” even while later reconciliation continues.

### Acceptance Criteria
- [ ] Initial indexing starts automatically after permission approval.
- [ ] Search is disabled only while no metadata index exists.
- [ ] Search placeholder is “Indexing…” during initial no-index build.
- [ ] Bottom-right status shows only spinner plus “Indexing” unless a user operation is active.
- [ ] After initial index exists, search is enabled with placeholder “Search”.

### Verification
- [ ] Add Rust tests for metadata row creation and index-state transitions.
- [ ] Add Angular tests for search disabled/enabled placeholders and indexing status.
- [ ] Manually run with a clean profile and watch initial indexing state transition.

## Task 15: Implement indexing exclusions, cloud safety, watchers, and reconciliation
Priority: P0
Phase: Indexing, search, alternate views, thumbnails
Depends on: Task 14
Requirements: REQ-012, REQ-013, REQ-014, REQ-027, REQ-029

### Goal
Keep the metadata index relevant and fresh without indexing noisy internals or forcing cloud downloads.

### Implementation Notes
- Define default excluded folders for system/app/program internals, dependency directories, VCS folders, build outputs, virtual environments, and package caches.
- Apply exclusions only to indexing/search, not browsing.
- Detect common cloud folders and index metadata without content reads that force downloads.
- Add filesystem watchers while the app is open.
- Add startup reconciliation that prunes deleted paths and updates changed/new paths.
- Report indexing failures non-blockingly with recovery actions.

### Acceptance Criteria
- [ ] Excluded directories do not enter search results by default.
- [ ] Excluded directories remain visible while browsing.
- [ ] Deleted paths are pruned from the index.
- [ ] Changed and new paths are updated after watcher events or startup reconciliation.
- [ ] Cloud-only files are not downloaded by indexing.
- [ ] Indexing failures appear in activity UI without crashing the app.

### Verification
- [ ] Add Rust tests for default exclusion matching and browsing/indexing distinction.
- [ ] Add Rust tests for reconciliation pruning and update behavior using temporary directories.
- [ ] Manually create, rename, and delete files while the app is open and after relaunch.

## Task 16: Implement fuzzy search backend and ranking
Priority: P0
Phase: Indexing, search, alternate views, thumbnails
Depends on: Task 15
Requirements: REQ-015, REQ-016, REQ-029

### Goal
Provide fast global fuzzy metadata search with Finder-like result prioritization.

### Implementation Notes
- Query the SQLite metadata index for exact, prefix, substring, and fuzzy matches.
- Rank folders before files, then exact match, prefix match, fuzzy score, recently opened boost, and modified-time boost.
- Return Name, Path, Date Modified, Size, and Kind fields for search rows.
- Support cancellation or stale-result protection for debounced frontend requests.
- Keep search local-only.

### Acceptance Criteria
- [ ] Partial and imperfect queries return matching files and folders.
- [ ] Folders rank before comparable files.
- [ ] Exact and prefix matches rank above weaker fuzzy matches.
- [ ] Recently opened and recently modified items receive appropriate boosts.
- [ ] Results include path context.

### Verification
- [ ] Add Rust tests for exact, prefix, substring, fuzzy, folder-first, recency, and modified ranking cases.
- [ ] Add command tests for empty query and no-result behavior.
- [ ] Manually search for known nested files and folders.

## Task 17: Integrate search UI, navigation behavior, and Reveal in Folder
Priority: P0
Phase: Indexing, search, alternate views, thumbnails
Depends on: Task 16
Requirements: REQ-009, REQ-015, REQ-016, REQ-018, REQ-026, REQ-029

### Goal
Make search feel live, reversible, and useful for navigation.

### Implementation Notes
- Add toolbar search input with 100–150ms debounce.
- Replace main file list with search results while search is active.
- Force search results into List view with Name, Path, Date Modified, Size, and Kind columns.
- Clearing search, pressing Escape, or pressing Back exits search.
- Treat active search state as a navigation-history state but do not restore it on relaunch.
- Double-clicking a result folder opens it and exits search.
- Double-clicking a result file opens it with default app while keeping results visible.
- Implement Reveal in Folder for search results by navigating to the containing folder and selecting the item.

### Acceptance Criteria
- [ ] Typing in search updates results after debounce.
- [ ] Search results replace the current directory view and use List layout.
- [ ] Escape, clear, and Back return to browsing naturally.
- [ ] Folder result activation exits search and navigates to the folder.
- [ ] File result activation opens the file and leaves results visible.
- [ ] Reveal in Folder navigates to the containing folder and selects the item.

### Verification
- [ ] Add Angular tests for debounce, forced List result view, exit triggers, and result activation.
- [ ] Add integration tests around search navigation state where practical.
- [ ] Manually search, open a folder result, open a file result, and reveal a nested file.

## USER-TEST Checkpoint 3: Views, thumbnails, indexing, and search

### What to Verify
- List, Grid/Icon, Column, and Gallery views are usable.
- Initial indexing disables search only until metadata exists.
- Search is fast, local, fuzzy, and ranks useful results near the top.
- Cloud placeholders are not force-downloaded by indexing or thumbnails.

### Happy Path
- Launch with a clean profile, wait for indexing to complete, search for a nested file with an imperfect query, open a folder result, and reveal a file result.

### Edge/Error Cases
- Search while indexing reconciliation is running after an initial index exists.
- Delete a file externally and confirm it disappears after reconciliation.
- Browse and search near excluded folders to confirm browsing and indexing differ.

### Regression Checks
- Search results always use List view.
- Search is not restored after relaunch.
- Indexing status remains minimal unless a user operation takes priority.

### Stop/Go Criteria
- Continue only if search is trustworthy enough to use during file operations and settings testing.

## Task 18: Implement core file operation backend
Priority: P0
Phase: File operations and interaction depth
Depends on: Task 9, Task 15
Requirements: REQ-014, REQ-021, REQ-029, REQ-030

### Goal
Make Frogger a real file manager by supporting safe core filesystem mutations.

### Implementation Notes
- Implement commands for New Folder, Rename, Move to Trash/Recycle Bin, Copy, Cut/Move, Paste, and default app opening.
- Delete key and toolbar Trash/Delete must only move to Trash/Recycle Bin.
- Add conflict detection for overwrite and merge cases.
- Add extension-change confirmation for renames where relevant.
- Quick successful operations update UI silently.
- Failed operations return visible, actionable errors and record recent failures.
- File operations should update directory views, Recents where applicable, and metadata index state.

### Acceptance Criteria
- [ ] New Folder creates a real folder in the current directory.
- [ ] Rename changes filesystem name and refreshes UI/index state.
- [ ] Trash/Delete moves items to Trash/Recycle Bin only.
- [ ] Copy/Paste and Cut/Paste perform expected filesystem changes.
- [ ] Conflicts and extension changes request confirmation.
- [ ] Successful quick operations do not show noisy success toasts.
- [ ] Failures are visible and recoverable.

### Verification
- [ ] Add Rust tests for create folder, rename, copy, move, trash behavior where platform test environment supports it, conflicts, and errors.
- [ ] Add Angular tests for operation command invocation and error rendering.
- [ ] Manually perform each operation in a temporary folder.

## Task 19: Implement clipboard, Open With, and OS drag/drop
Priority: P0
Phase: File operations and interaction depth
Depends on: Task 18
Requirements: REQ-021, REQ-022, REQ-023, REQ-029

### Goal
Integrate Frogger with standard OS workflows for file movement and app opening.

### Implementation Notes
- Implement Copy, Cut, and Paste through app state and OS clipboard integration where Tauri supports safe file references.
- Implement Open With using the OS chooser/dialog where possible.
- Implement inbound drops from OS/apps into Frogger.
- Implement outbound drags from Frogger to OS/apps.
- Respect platform move/copy conventions and modifier overrides.
- Ensure drag/drop operations report progress and failures through the activity system.

### Acceptance Criteria
- [ ] Copy/Cut/Paste shortcuts and context actions work on selected items.
- [ ] Open With invokes a platform chooser where available or a graceful platform-specific fallback.
- [ ] Files can be dropped into Frogger from the OS.
- [ ] Files can be dragged from Frogger into the OS or another app.
- [ ] Modifier keys produce platform-appropriate copy/move behavior.

### Verification
- [ ] Add tests for clipboard state transitions and paste target validation.
- [ ] Add manual OS drag/drop tests on each supported platform available to the team.
- [ ] Verify failed drops produce visible errors and recent failure entries.

## Task 20: Implement context menus, Get Info, Copy Path, Go to Folder, and Open in Terminal
Priority: P0
Phase: File operations and interaction depth
Depends on: Task 18
Requirements: REQ-022, REQ-025, REQ-030, REQ-029

### Goal
Expose common file-manager actions near the pointer and through standard commands.

### Implementation Notes
- Context menus include Open, Open With, Rename, Move to Trash, Copy, Cut, Paste, New Folder, Get Info, and Copy Path.
- Disable unavailable actions based on selection and target context.
- Get Info is an app-owned modal or panel with name, kind, full path, size, dates, icon, and permissions/read-only status.
- Copy Path copies absolute filesystem paths only.
- Go to Folder opens via menu/shortcut and validates paths with inline errors.
- Open in Terminal opens the current or selected folder in the system default terminal.
- Do not implement general Reveal in Finder/File Explorer outside search-result Reveal in Folder.

### Acceptance Criteria
- [ ] Context menus expose only functional Phase 1 actions.
- [ ] Disabled states match selection and clipboard state.
- [ ] Get Info displays required metadata.
- [ ] Copy Path places absolute paths on the clipboard.
- [ ] Go to Folder navigates valid paths and shows inline errors for invalid paths.
- [ ] Open in Terminal launches the selected/current folder in the default terminal.

### Verification
- [ ] Add Angular tests for menu item visibility, disabled states, Get Info rendering, and Go to Folder errors.
- [ ] Add Rust tests for path validation and terminal command construction where practical.
- [ ] Manually test context actions on files, folders, empty space, Recents, and search results.

## Task 21: Implement activity status, operation priority, failures, and recovery
Priority: P0
Phase: File operations and interaction depth
Depends on: Task 14, Task 18
Requirements: REQ-014, REQ-021, REQ-029

### Goal
Make background and user-initiated work visible without noisy alerts.

### Implementation Notes
- Bottom-right status normally shows item count or selected count.
- Indexing shows spinner plus “Indexing” only when no user operation has priority.
- User file operations take bottom-status priority over indexing.
- Clicking the status area opens an activity popover.
- Popover shows active operations, recent failed operations, minimal indexing state, and retry/recovery actions for indexing failures.
- Keep successful quick operations silent.

### Acceptance Criteria
- [ ] Status priority is user operation first, indexing second, idle count third.
- [ ] Activity popover lists active operations and recent failures.
- [ ] Indexing failures show non-blocking recovery actions.
- [ ] Successful quick operations do not create noisy history.
- [ ] Long operations show progress or active state.

### Verification
- [ ] Add Angular tests for status priority and activity popover rendering.
- [ ] Add Rust tests for activity persistence of recent failures.
- [ ] Manually trigger long copy and indexing failure scenarios where possible.

## Task 22: Implement undo for reliable rename and trash actions
Priority: P1
Phase: File operations and interaction depth
Depends on: Task 18, Task 21
Requirements: REQ-024, REQ-029

### Goal
Allow recovery from common mistakes where the platform behavior is reliable.

### Implementation Notes
- Record undo records for successful rename operations.
- Record undo records for move-to-trash when the platform exposes enough restoration information.
- Expose undo via platform shortcut and menu command.
- Show a clear error if an undo cannot be completed because the path changed, permissions changed, or platform data is unavailable.
- Do not promise full undo for every file operation.

### Acceptance Criteria
- [ ] Rename can be undone when source and target paths are still valid.
- [ ] Move-to-trash can be undone on platforms where reliable restore is available.
- [ ] Undo failures are visible and do not corrupt UI state.
- [ ] Unsupported undo cases are not presented as guaranteed.

### Verification
- [ ] Add Rust tests for rename undo success and invalidated undo failure.
- [ ] Add Angular tests for undo command availability and error rendering.
- [ ] Manually test rename undo and trash undo on supported platforms.

## Task 23: Implement keyboard shortcuts, keyboard navigation, multi-select, and drag marquee
Priority: P0
Phase: File operations and interaction depth
Depends on: Task 10, Task 18
Requirements: REQ-018, REQ-021, REQ-027, REQ-028, REQ-029

### Goal
Make Frogger fast and familiar for keyboard and batch workflows.

### Implementation Notes
- Map platform-native shortcuts for search, settings, new folder, rename, delete/trash, copy, cut, paste, select all, close tab, new tab, new window, Go to Folder, undo, and Escape behavior.
- Implement arrow navigation and Enter activation.
- Implement Delete/Backspace behavior according to platform convention while still moving to Trash only.
- Implement click, Cmd/Ctrl-click, Shift-click, and drag marquee multi-select.
- Add shortcut settings display that reflects actual active mappings.

### Acceptance Criteria
- [ ] Keyboard navigation works in List, Grid, Column, and search results where applicable.
- [ ] Standard selection patterns work for single and multi-select.
- [ ] Shortcuts match macOS and Windows conventions.
- [ ] Delete/Backspace moves to Trash/Recycle Bin only.
- [ ] Escape exits search or preview before clearing selection.

### Verification
- [ ] Add Angular tests for shortcut mapping, selection ranges, toggle selection, select all, and Escape priority.
- [ ] Manually verify shortcuts on macOS and Windows if available.
- [ ] Regression-check that Windows uses Windows-native shortcuts despite macOS-inspired visuals.

## USER-TEST Checkpoint 4: File operations and power-user interactions

### What to Verify
- Core file operations work on real files and folders.
- Context menus and shortcuts expose the same reliable actions.
- Drag/drop works with the OS.
- Activity status explains long work and failures without noise.

### Happy Path
- Create a folder, rename it, copy a file into it, move another file, open a file, copy its path, view Get Info, move it to Trash, and undo a supported action.

### Edge/Error Cases
- Rename a file to a different extension and confirm warning behavior.
- Paste into a destination with a conflict and verify confirmation.
- Trigger a permission failure and verify activity/recovery behavior.

### Regression Checks
- Delete never performs permanent delete.
- General Reveal in Finder/File Explorer is not available outside search-result Reveal in Folder.
- Quick successes do not produce noisy alerts.

### Stop/Go Criteria
- Continue only if file operations are safe enough for broader settings and preview testing.

## Task 24: Implement Spacebar preview and hybrid preview renderers
Priority: P0
Phase: Previews, settings, polish, and hardening
Depends on: Task 12, Task 13, Task 23
Requirements: REQ-013, REQ-019, REQ-029, REQ-030

### Goal
Provide reliable preview behavior for common files and graceful fallback for unsupported types.

### Implementation Notes
- Spacebar opens and closes preview for the current selection.
- Implement web-rendered previews for images, video, audio, text/code, and PDFs where feasible.
- Unsupported documents show icon and metadata fallback.
- Preview must not trigger cloud-only file downloads.
- Gallery view reuses preview infrastructure for the large preview area.
- No preview pane is shown by default and no preview toolbar button is added.

### Acceptance Criteria
- [ ] Spacebar preview works for supported image, video, audio, text/code, and PDF files.
- [ ] Unsupported files show icon and metadata rather than a broken view.
- [ ] Preview closes predictably with Spacebar or Escape.
- [ ] Cloud-only files are not downloaded solely for preview.
- [ ] No preview pane or preview toolbar button appears by default.

### Verification
- [ ] Add Angular tests for preview open/close, supported renderer routing, and fallback rendering.
- [ ] Add Rust tests for safe preview metadata/read decisions.
- [ ] Manually preview representative supported and unsupported files.

## Task 25: Implement full functional Settings screen and app menu access
Priority: P0
Phase: Previews, settings, polish, and hardening
Depends on: Task 11, Task 15, Task 23, Task 24
Requirements: REQ-006, REQ-008, REQ-012, REQ-013, REQ-027, REQ-028, REQ-029

### Goal
Ship settings that are trustworthy because every visible preference changes real behavior.

### Implementation Notes
- Settings are accessed from app menu and shortcut, not a toolbar gear.
- Sections include Appearance, Shortcuts, Indexing, AI model prep, Previews, Privacy, Sidebar, Restore/session, hidden files, file extensions, folders-first sorting, and path bar visibility.
- Every shown setting must read/write SQLite and update the running app.
- Indexing settings include locations/exclusions and local-only privacy explanation.
- AI prep settings can describe local semantic-search preparation without enabling active semantic search.
- Restore/session settings control relevant restore behavior without breaking safe Home fallback.

### Acceptance Criteria
- [ ] Settings opens from menu and shortcut.
- [ ] Every visible setting persists and changes actual behavior.
- [ ] Hidden files, file extensions, folders-first, path bar, sidebar sections, indexing exclusions, previews, theme, and restore settings work.
- [ ] AI model prep does not enable active semantic search.
- [ ] Settings UI has no fake controls.

### Verification
- [ ] Add Angular settings UI tests for each visible setting.
- [ ] Add Rust persistence tests for settings read/write and default values.
- [ ] Manually change every setting, close settings, relaunch, and verify behavior persists.

## Task 26: Polish appearance, custom chrome, dark mode, and cross-platform conventions
Priority: P0
Phase: Previews, settings, polish, and hardening
Depends on: Task 7, Task 23, Task 25
Requirements: REQ-001, REQ-026, REQ-028, REQ-030

### Goal
Make Frogger feel like a calm high-performance tool with equal-quality light and dark modes.

### Implementation Notes
- Implement system appearance following with polished light and dark theme tokens.
- Create translucent/material-inspired sidebar styling where platform rendering allows it, with solid fallback where needed.
- Keep macOS-inspired custom chrome on Windows while preserving Windows-native shortcuts and file operation conventions.
- Tune typography, spacing, row heights, focus states, disabled states, and skeletons for a native desktop feel.
- Ensure app name is not used as main UI branding.

### Acceptance Criteria
- [ ] App follows system light/dark appearance by default.
- [ ] Dark mode is visually complete, not an afterthought.
- [ ] Custom chrome works on macOS and Windows visual targets.
- [ ] Windows shortcuts and file conventions remain Windows-native.
- [ ] Focus and disabled states are accessible and clear.

### Verification
- [ ] Add visual/component checks for light and dark theme class/state rendering.
- [ ] Manually test appearance switching at runtime.
- [ ] Manually verify shortcut conventions on each available platform.

## Task 27: Harden cloud-folder safety, privacy, and local-only semantics
Priority: P0
Phase: Previews, settings, polish, and hardening
Depends on: Task 13, Task 15, Task 24, Task 25
Requirements: REQ-012, REQ-013, REQ-027, REQ-029

### Goal
Ensure indexing, thumbnails, previews, and future AI prep preserve privacy and avoid unwanted downloads.

### Implementation Notes
- Centralize cloud-placeholder detection and safe-read decisions so indexing, thumbnails, and previews share the same policy.
- Settings privacy section states metadata/indexing and future embeddings are local-only.
- Add tests around simulated placeholder metadata and safe no-content-read behavior.
- Ensure indexing locations reflect user-interactable folders while excluding internal app/system/program files.
- Keep semantic embeddings inactive in Phase 1.

### Acceptance Criteria
- [ ] Indexing does not read file content for metadata indexing.
- [ ] Thumbnails and previews skip cloud-only files unless the user explicitly opens them through normal OS behavior.
- [ ] Privacy settings accurately describe local-only behavior.
- [ ] Semantic search is not active, while the architecture remains prepared for later local embeddings.

### Verification
- [ ] Add Rust tests for shared safe-read policy decisions.
- [ ] Manually test with cloud folders or simulated cloud-placeholder files.
- [ ] Inspect settings and toolbar to confirm AI remains inactive with tooltip-only behavior.

## Task 28: Complete automated test coverage and release hardening
Priority: P0
Phase: Previews, settings, polish, and hardening
Depends on: Task 24, Task 25, Task 26, Task 27
Requirements: REQ-029, REQ-030

### Goal
Lock Phase 1 behavior with stable tests and final regression checks.

### Implementation Notes
- Add or finish tests for session persistence, settings, browsing, indexing, search, file operations, undo, drag/drop contracts, previews, thumbnail cache, sidebar, recents, activity, shortcuts, context menus, and settings UI.
- Prefer stable module interfaces and user-visible outcomes over implementation details.
- Add test fixtures for temporary filesystem structures and large-directory performance checks.
- Run production build and Tauri build checks.
- Verify out-of-scope controls and features are absent.

### Acceptance Criteria
- [ ] All P0 modules have automated tests for external behavior.
- [ ] Production Angular build succeeds.
- [ ] Rust tests pass.
- [ ] Tauri dev app launches without starter artifacts.
- [ ] Out-of-scope features remain absent or explicitly inactive where specified.

### Verification
- [ ] Run `bun run build`.
- [ ] Run Angular test command configured in Task 1.
- [ ] Run `cd src-tauri && cargo test`.
- [ ] Run `bun run tauri dev` and complete a final manual smoke test.

## USER-TEST Checkpoint 5: Phase 1 acceptance smoke test

### What to Verify
- Frogger launches into a polished Finder-style file manager.
- Restore, browsing, sidebar, views, indexing, search, previews, operations, settings, activity, and shortcuts all work together.
- The app feels like a tool, not a branded dashboard.

### Happy Path
- Start from a clean profile, approve access, browse Home, switch views, index, search, open files, create/rename/copy/trash items, preview files, adjust settings, open multiple tabs/windows, quit, and relaunch.

### Edge/Error Cases
- Deny access and recover.
- Restore with missing folders.
- Trigger file operation conflict and permission failure.
- Use a huge folder.
- Use cloud-placeholder files.
- Change system theme.

### Regression Checks
- Search disabled only during initial no-index build.
- Folder browsing remains truthful even for index-excluded folders.
- No fake Share, Tags, Grouping, permanent delete, dedicated preview toolbar button, settings toolbar gear, or main UI branding appears.
- Settings shown in Phase 1 all change real behavior.

### Stop/Go Criteria
- Phase 1 is ready for release hardening only when the smoke test passes without data loss, broken restoration, or fake visible functionality.

## User Validation Checkpoints

| Checkpoint | After Tasks | Focus | Stop/Go Summary |
|---|---|---|---|
| USER-TEST Checkpoint 1 | 1-4 | Launch, permissions, persistence foundation | Clean profile reaches Home or denied-access recovery. |
| USER-TEST Checkpoint 2 | 5-11 | Shell, restore, sidebar, List browsing | List browsing and session restoration are reliable. |
| USER-TEST Checkpoint 3 | 12-17 | Views, thumbnails, indexing, search | Search and indexing are trustworthy and local. |
| USER-TEST Checkpoint 4 | 18-23 | File operations and power interactions | Operations are safe, recoverable, and non-noisy. |
| USER-TEST Checkpoint 5 | 24-28 | Full Phase 1 smoke test | Integrated Phase 1 behavior passes without fake features or data-loss risks. |

## Risks and Open Questions

- **Filesystem permission model:** Tauri permissions and macOS Full Disk Access behavior may require platform-specific recovery copy and fallback flows.
- **Native macOS titlebar tabs:** Tauri support may be limited; custom tabs must remain acceptable if native tabs are unreliable.
- **Cloud placeholder detection:** Providers differ. A conservative shared safe-read policy is required to avoid accidental downloads.
- **Trash undo reliability:** Platform APIs may not expose robust restoration for every trash operation. The UI must only offer undo where reliable.
- **Open With chooser:** OS chooser availability may vary by platform; fallback behavior must be explicit and tested.
- **Huge-folder performance:** Virtualization plus streaming contracts must be validated early with generated test folders.
- **SQLite concurrency:** Indexing, watchers, file operations, and UI reads need careful transaction and connection management.
- **Scope pressure:** Settings, preview formats, drag/drop, and multi-window behavior are broad. Do not compensate by adding fake controls.
- **Windows visual/behavior split:** The app is macOS-inspired visually but must keep Windows keyboard and file-operation conventions.

## Suggested First Command / Next Step

Start with Task 1 by updating project metadata/dependencies and confirming the starter still builds:

```bash
bun run build && cd src-tauri && cargo test
```
