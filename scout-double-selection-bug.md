# Code Context

## Files Retrieved
1. `src/app/app.component.html` (lines 30-88) - inline sidebar template for Recents, Favorites, and Locations; applies active class.
2. `src/app/app.component.ts` (lines 38-69, 427-485, 799-880) - component state, sidebar item construction, active predicate, navigation, path normalization/deduping.
3. `src/app/app.component.css` (lines 130-162) - sidebar highlight styling; no focus style causing the duplicate highlight.
4. `src/app/core/frogger-api.types.ts` (lines 1-134) - frontend data model for sidebar/bootstrap/window/tab state.
5. `src-tauri/src/commands.rs` (lines 1500-1704) - backend sidebar state loader and detected Locations model.

## Key Code

Sidebar template applies the same predicate independently to each section:

```html
<!-- src/app/app.component.html:59-66 -->
@for (favorite of favoriteSidebarItems(state); track favorite.path) {
  <button
    class="sidebar-item"
    [class.sidebar-item--active]="isActivePath(favorite.path)"
    [attr.aria-current]="isActivePath(favorite.path) ? 'page' : null"
    (click)="openSidebarPath(favorite.path, favorite.label)"
  >
```

```html
<!-- src/app/app.component.html:77-84 -->
@for (location of locationSidebarItems(state); track location.path) {
  <button
    class="sidebar-item"
    [class.sidebar-item--active]="isActivePath(location.path)"
    [attr.aria-current]="isActivePath(location.path) ? 'page' : null"
    (click)="openSidebarPath(location.path, location.label)"
  >
```

Selection predicate is exact normalized path equality; no `startsWith`/`includes` issue:

```ts
// src/app/app.component.ts:451-453
isActivePath(path: string): boolean {
  return this.normalizePath(this.session.activeTab()?.path) === this.normalizePath(path);
}

// src/app/app.component.ts:872-874
private normalizePath(path: string | null | undefined): string {
  return (path ?? "").replace(/[/\\]+$/, "").toLowerCase();
}
```

The colliding items are constructed on the frontend:

```ts
// src/app/app.component.ts:463-473
favoriteSidebarItems(state: AppBootstrap): SidebarNavItem[] {
  const home = state.access.homeDir;
  const defaults: SidebarNavItem[] = home
    ? [
        { label: this.folderName(home), path: home, icon: "icon-home" },
        { label: "Desktop", path: this.joinPath(home, "Desktop"), icon: "icon-desktop" },
        ...
      ]
    : [];
```

For `/Users/lukesmac`, `folderName(home)` returns `lukesmac`, so Favorites contains:

- label: `lukesmac`
- path: `/Users/lukesmac`
- icon: `icon-home`

```ts
// src/app/app.component.ts:477-485
locationSidebarItems(state: AppBootstrap): SidebarNavItem[] {
  const home = state.access.homeDir;
  const defaults: SidebarNavItem[] = home
    ? [
        { label: "iCloud Drive", path: this.joinPath(home, "Library/Mobile Documents/com~apple~CloudDocs"), icon: "icon-cloud" },
        { label: "AirDrop", path: home, icon: "icon-airdrop" },
        { label: "Network", path: "/Network", icon: "icon-network" },
      ]
```

So Locations contains:

- label: `AirDrop`
- path: `/Users/lukesmac`
- icon: `icon-airdrop`

Deduplication only happens within each section list, not across Favorites and Locations:

```ts
// src/app/app.component.ts:858-867
private uniqueSidebarItems(items: SidebarNavItem[]): SidebarNavItem[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    const key = this.normalizePath(item.path);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
```

CSS only highlights `.sidebar-item--active`; no sidebar focus/keyboard class was found:

```css
/* src/app/app.component.css:155-162 */
.sidebar-item:hover { background: rgba(0, 0, 0, 0.045); }
.sidebar-item--active {
  color: #006ce8;
  background: rgba(0, 0, 0, 0.065);
}
```

Backend Locations includes a real Home item, but the frontend filters it out and then adds the AirDrop default pointing at Home:

```rust
// src-tauri/src/commands.rs:1611-1621
fn detect_locations(home_dir: Option<String>) -> Vec<SidebarItem> {
  ...
  if let Some(home) = home_dir {
    push_location(..., "home".to_string(), "Home".to_string(), home.clone(), SidebarItemType::Home);
```

```ts
// src/app/app.component.ts:488-491
const detected = state.sidebar.locations
  .filter((location) => location.itemType !== "home")
  .map((location) => this.toSidebarNavItem(location));
return this.uniqueSidebarItems([...detected, ...defaults]);
```

## Architecture

`AppComponent` owns the entire file-manager UI; there is no separate sidebar component under `src/app`. Bootstrap data comes from Tauri via `FroggerApiService.bootstrap()`, then `SessionStoreService` stores the active window/tab. The sidebar active state is derived only from `session.activeTab()?.path` via `isActivePath()`.

When clicking Favorites `lukesmac`, `openSidebarPath()` calls `navigateToPath('/Users/lukesmac', 'lukesmac')`, which updates the active tab path to home. Angular then evaluates every sidebar button. Because Locations `AirDrop` also has path `/Users/lukesmac`, both buttons get `.sidebar-item--active` and `aria-current="page"`.

This is not a per-section default, signal-sharing, route matching, or keyboard/focus issue. It is a path collision.

## Findings

- Colliding item 1: Favorites default home item `lukesmac`, path `/Users/lukesmac` (`state.access.homeDir`).
- Colliding item 2: Locations default `AirDrop`, path `/Users/lukesmac` (`state.access.homeDir`).
- Cause: active state is path-based, and both items carry the same normalized path.
- Confidence: high.
- Gap/open question: whether `AirDrop` is meant to be implemented as a real virtual/native target later. Currently it is a placeholder that navigates to Home, making it indistinguishable from the Home favorite.

## Recommended Fix Direction

Best fix: do not model AirDrop as `path: home`. Give it a distinct non-colliding identifier/path only if supported (for example a virtual `frogger://airdrop`) and handle navigation specially, or remove/disable AirDrop until it has a real action.

If the UI should allow duplicate paths across sections for other reasons, introduce stable sidebar item IDs and track active sidebar item by ID/section separately from active filesystem path. However, for this bug the simplest safe correction is changing/removing the AirDrop placeholder path.

## Start Here

Open `src/app/app.component.ts` first, specifically `favoriteSidebarItems()`, `locationSidebarItems()`, and `isActivePath()`. The collision is visible there without needing backend changes.
