# Code Context

## Files Retrieved
1. `src/app/app.component.html` (lines 57-68, 238-244, 367-376) - sidebar click bindings, error UI, breadcrumb rendering.
2. `src/app/app.component.ts` (lines 170-176, 452-458, 507-521, 670-688, 752-781, 891-947, 950-960) - breadcrumb source, sidebar item construction, navigation, directory load, path normalization.
3. `src/app/core/frogger-api.service.ts` (lines 8-35) - Tauri invoke methods for bootstrap and `list_directory`.
4. `src/app/core/frogger-api.types.ts` (lines 22-49, 91-119) - `AppBootstrap`, access state, sidebar item, directory list request types.
5. `src/app/core/session-store.service.ts` (lines 36-44, 132-166) - bootstrap session initialization and active tab path update.
6. `src-tauri/src/commands.rs` (lines 50-84, 169-189, 459-511, 897-909, 949-975, 1500-1588, 1611-1684) - bootstrap/home-dir source, list command, filesystem error mapping, sidebar DB/detected items.
7. `src-tauri/src/errors.rs` (lines 4-34) - serialized `CommandError` shape.
8. `src-tauri/capabilities/default.json` (lines 1-10) and `src-tauri/tauri.conf.json` (lines 1-38) - Tauri capabilities/security config.
9. `src-tauri/Cargo.toml` (lines 18-33) and `src-tauri/src/lib.rs` (lines 8-63) - plugins and command registration.

## Key Code

### Favorites/home sidebar construction
`src/app/app.component.ts` lines 507-521:
```ts
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
```
The `lukesmac` default favorite is not hardcoded and not read from favorites DB. It is derived from `state.access.homeDir`.

`src-tauri/src/commands.rs` lines 949-950:
```rust
fn home_dir_string() -> Option<String> {
    directories::UserDirs::new().map(|dirs| dirs.home_dir().to_string_lossy().into_owned())
}
```
`bootstrap_app` passes this into both `platform.home_dir` and `access.home_dir` at lines 58-81, after `detect_file_access()` checks `Path::new(path).is_dir()` and `read_dir().is_ok()` at lines 953-975.

Pinned favorites are loaded from SQLite in `load_favorites()` (`src-tauri/src/commands.rs` lines 1572-1588), but only if `Path::new(&path).is_dir()`. I checked the current app DB at `~/Library/Application Support/com.lukesmac.frogger/frogger.sqlite3`: `favorites` is empty; the active tab is `/Users/lukesmac`. That makes a stale pinned Favorite unlikely for this local state.

### Click/navigation flow
`src/app/app.component.html` lines 57-68 renders Favorites buttons and calls:
```html
(click)="openSidebarPath(favorite.path, favorite.label)"
```

`src/app/app.component.ts` lines 456-458:
```ts
openSidebarPath(path: string, label: string): void {
  void this.navigateToPath(path, label);
}
```

`navigateToPath()` (`src/app/app.component.ts` lines 670-688) optionally loads persisted folder view state, ignores errors, then updates the active tab path:
```ts
folderState = this.isRecentsPath(path) ? null : await this.api.getFolderViewState(path);
...
this.session.updateActiveTabPath(path, label, folderState);
```

`SessionStoreService.updateActiveTabPath()` (`src/app/core/session-store.service.ts` lines 132-166) mutates the active tab. An Angular effect in `app.component.ts` lines 180-216 observes `activeTab.path` and calls `loadDirectory(...)`.

`loadDirectory()` (`src/app/app.component.ts` lines 752-781) calls:
```ts
const listing = await this.api.listDirectory(path, sort, foldersFirst, hiddenFilesVisible, fileExtensionsVisible);
```
`FroggerApiService.listDirectory()` invokes Tauri command `list_directory` with `{ request }` (`src/app/core/frogger-api.service.ts` lines 20-35).

### Exact error-producing path
UI title comes from `src/app/app.component.html` lines 238-244:
```html
<strong>Folder unavailable</strong>
<span>{{ listingError() }}</span>
```
`listingError` is set in `loadDirectory()` catch block (`src/app/app.component.ts` lines 775-781) via `toErrorMessage(error)`, which extracts `error.message` (`src/app/app.component.ts` lines 950-960).

The exact string `The requested path no longer exists.` is produced only in `src-tauri/src/commands.rs` lines 897-902:
```rust
fn fs_access_error(path: &Path, error: io::Error) -> CommandError {
    match error.kind() {
        io::ErrorKind::NotFound => CommandError::missing_path(
            "The requested path no longer exists.",
            Some(path.to_string_lossy().into_owned()),
        ),
```
For `list_directory`, this can be triggered at `src-tauri/src/commands.rs` lines 467 or 476:
```rust
let metadata = std::fs::metadata(&target).map_err(|error| fs_access_error(&target, error))?;
...
let directory = std::fs::read_dir(&target).map_err(|error| fs_access_error(&target, error))?;
```
So the UI message means Rust received some `request.path`, converted it to `PathBuf`, and either `metadata()` or `read_dir()` returned `io::ErrorKind::NotFound`. The UI does not display `CommandError.details`, which would contain the exact path Rust failed on.

### Breadcrumb vs requested path
Breadcrumbs are computed only from `session.activeTab()?.path` (`src/app/app.component.ts` lines 170-176, 907-947). For absolute Unix paths, it always prepends `Macintosh HD` and then each component:
```ts
breadcrumbs.push({ label: "Macintosh HD", path: "/" });
let current = "";
for (const part of parts) {
  current = `${current}/${part}`;
  breadcrumbs.push({ label: part, path: current });
}
```
Because `navigateToPath()` updates `activeTab.path` before `list_directory` succeeds, the breadcrumb reflects the attempted path, not a confirmed listing. A visually valid breadcrumb (`Macintosh HD > Users > lukesmac`) does not prove `list_directory` was called with exactly `/Users/lukesmac`; hidden chars or a variant path could render nearly the same. However, with the current code path, the sidebar default home path and the requested path should be the same string (`state.access.homeDir`) unless a pinned favorite with same-looking label/path appears first.

### Tauri capabilities / fs scope
`src-tauri/capabilities/default.json` lines 5-9 grants only `core:default` and `opener:default`. `src-tauri/Cargo.toml` lines 18-33 includes `tauri-plugin-opener` only; no Tauri fs plugin. `list_directory` is a custom Rust command registered in `src-tauri/src/lib.rs` lines 45-62 and uses `std::fs` directly. Therefore Tauri fs scopes/capabilities are not involved in this directory read path. There are no `$HOME` scope rules that could reject home while allowing subfolders.

## Architecture
Bootstrap (`bootstrap_app`) derives home via `directories::UserDirs::new().home_dir()` and checks readability. Angular stores the bootstrap in `SessionStoreService`. The Favorites section merges pinned DB favorites first, then default favorites derived from `access.homeDir`. Clicking a sidebar favorite updates the active tab path; an effect calls `list_directory`; Rust uses `std::fs::metadata` and `std::fs::read_dir`; filesystem errors are mapped to serialized `CommandError`; Angular displays only `CommandError.message`.

## Findings / Hypothesis
- The home/lukesmac Favorites entry is generated from Tauri/Rust `directories::UserDirs`, not hardcoded and not from cached config, unless there is a pinned favorite shown before it.
- The exact failing condition is `std::fs::metadata(&PathBuf::from(request.path))` or `std::fs::read_dir(&target)` returning `NotFound` in `list_directory_impl`.
- Current local DB has no pinned favorites, and `/Users/lukesmac` exists/readable from this shell, so a stale favorite/config is not supported by the inspected state.
- Top hypothesis: the app is not actually failing on plain `/Users/lukesmac`; it is receiving a same-looking but different path (hidden character, stale active tab in another app data location/profile, or a path variant not visible in the breadcrumb). Confidence: medium-low, because the code should list `/Users/lukesmac` if that exact string reaches Rust.
- Less likely: a race/removal during `metadata`/`read_dir`, or different runtime environment/user than this shell. Tauri fs scope is very unlikely/not applicable.
- Main gap: the UI discards `CommandError.details`, so the exact failing path is not visible. Logging or surfacing `details` for `list_directory` would confirm the actual path immediately.

## Start Here
Open `src/app/app.component.ts` at `favoriteSidebarItems()` and `loadDirectory()` first. It shows both the generated home favorite path and the path sent to the Tauri `list_directory` command.