# Code Context

## Files Retrieved
1. `src-tauri/src/persistence.rs` (lines 142-219) - SQLite schema for `metadata_index` and `index_state`.
2. `src-tauri/src/commands.rs` (lines 169-191) - Tauri `list_directory` command entry point.
3. `src-tauri/src/commands.rs` (lines 407-455) - Recents virtual-folder listing path, which re-stats recent paths.
4. `src-tauri/src/commands.rs` (lines 457-568) - current local filesystem directory listing implementation.
5. `src-tauri/src/commands.rs` (lines 583-668) - per-entry metadata extraction and symlink handling.
6. `src-tauri/src/commands.rs` (lines 726-749) - in-memory sorting for every directory listing.
7. `src-tauri/src/commands.rs` (lines 1191-1221) - indexing state loader; only counts rows and maps status.
8. `src-tauri/src/models.rs` (lines 188-258, 273-313) - DTOs for directory listing, search result, and indexing state.
9. `src-tauri/src/lib.rs` (lines 8-58) - app setup and registered Tauri commands; no indexing/search command registered.
10. `src/app/core/frogger-api.service.ts` (lines 6-40) - frontend API exposes `listDirectory`; no search/indexing calls.
11. `src/app/app.component.ts` (lines 596-605, 785-853, 853-877) - UI uses indexing state only for placeholder/status; directory load and thumbnail fan-out.
12. `src/app/app.component.html` (lines 177-179, 413-416) - search box disabled while no initial index.
13. `src-tauri/Cargo.toml` (lines 18-31) - indexing/search-related crates are declared (`fuzzy-matcher`, `ignore`, `notify`, `walkdir`) but unused in `src-tauri/src`.
14. `src-tauri/src/commands.rs` (lines 1899-1921, 2130-2235) - current tests covering index-state count and directory listing basics.
15. `src-tauri/src/persistence.rs` (lines 235-366) - migration tests include metadata table existence and sample insert/read.
16. `docs/tasks-prd-frogger-phase-1-initial-file-manager.md` (lines 476-577) - planned indexing/search/watchers requirements are unchecked.

## Key Code

### Current persistence schema exists, but no builder/updater found
`src-tauri/src/persistence.rs` (lines 142-164):
```sql
CREATE TABLE IF NOT EXISTS metadata_index (... search_text TEXT NOT NULL, recent_boost REAL NOT NULL DEFAULT 0, modified_boost REAL NOT NULL DEFAULT 0);
CREATE INDEX IF NOT EXISTS idx_metadata_search_text ON metadata_index(search_text);
```
`src-tauri/src/persistence.rs` (lines 166-219) creates `index_state` and inserts `('metadata', 'not_started', 0)`.

### Current directory browsing is direct filesystem scan, not index-backed
`src-tauri/src/commands.rs` (lines 169-191) routes `list_directory` either to Recents or direct `list_directory_impl`.

`src-tauri/src/commands.rs` (lines 499-544) does:
- `std::fs::read_dir(&target)`
- loops all entries
- calls `file_entry_from_dir_entry` for each
- then `sort_entries(&mut entries, ...)`
- only after full scan/sort applies cursor/limit.

`src-tauri/src/commands.rs` (lines 599-665) per entry calls `symlink_metadata`; symlinks call `metadata` and `read_link`; then reads len, modified, created, permissions.

### Search/indexing models are placeholders
`src-tauri/src/models.rs` (lines 273-313) defines `SearchResult`, `SearchMatchReason`, `IndexingState`, `IndexingStatus`.
`src-tauri/src/lib.rs` (lines 42-57) registered commands do not include search, start-index, reconcile, watcher, or index update commands.
`grep` found no `WalkDir`, `fuzzy_match`, `ignore::`, or `notify::` use under `src-tauri/src`.

### Frontend only gates UI based on stored state
`src/app/app.component.ts` (lines 596-605): `searchPlaceholder` returns `Indexing…` until `state.indexing.hasInitialIndex`; `statusLabel` shows `Indexing` while false.
`src/app/app.component.html` (lines 177-179): search input is disabled while `!state.indexing.hasInitialIndex`, but has no `(input)` handler or search command call.

### Likely bottlenecks in current local browsing
1. Full scan before pagination: `list_directory_impl` always reads all entries, builds all metadata, and sorts all entries before `.skip().take()` (lines 499-563). `limit` only shrinks response payload, not filesystem work.
2. Per-entry metadata calls: at least one stat (`symlink_metadata`) per entry plus multiple metadata property calls; symlinks add `metadata` and `read_link` (lines 599-665).
3. In-memory sort allocates lowercase names during comparisons (`to_ascii_lowercase()` inside comparator) (lines 726-747), causing repeated allocation on large folders.
4. Recents virtual folder re-stats every recent path and only pages after all are processed (lines 407-455).
5. Thumbnail loading fans out up to 160 concurrent `getThumbnail` invokes after listing (app component lines 853-877), which can contend with browsing and CPU/disk, especially on first generation.
6. Bootstrap counts all index rows every app launch (`SELECT COUNT(*) FROM metadata_index`, commands lines 1200-1204). Probably cheap for small DB, but can become noticeable with a large metadata index unless stored in `index_state`.

## Architecture

Current flow:
1. App setup opens/migrates SQLite (`src-tauri/src/lib.rs` lines 11-21).
2. Frontend calls `bootstrap_app` through `FroggerApiService.bootstrap()`; backend loads settings, windows, sidebar, and `load_indexing_state` (`commands.rs` lines 43-85).
3. UI disables search if `index_state.has_initial_index` is false; there is currently no automatic transition because no indexer writes rows/state.
4. Directory browsing calls `FroggerApiService.listDirectory()` -> Tauri `list_directory` -> direct filesystem scan or Recents virtual folder.
5. After listing, frontend optionally fires thumbnail requests for first 160 image entries.

Dependencies/constraints:
- SQLite schema is in place for index/search state.
- `fuzzy-matcher`, `ignore`, `notify`, `walkdir`, `tokio` are already dependencies, but indexing/search code is not implemented.
- Requirements explicitly say indexing exclusions apply only to indexing/search, not browsing (`docs/...` lines 520-523), and cloud-only files must not be hydrated/downloaded.
- Search must be local-only; semantic/AI search is out of scope for active functionality.

## Tests / Benchmarks

Existing tests:
- `commands.rs` lines 1899-1921: `load_indexing_state_counts_metadata_rows` only verifies counting manually inserted metadata rows and status mapping.
- `commands.rs` lines 2130-2235: directory listing tests for hidden-file filtering, metadata, pagination response fields, missing-path error.
- `persistence.rs` lines 235-366: migration/table existence and sample metadata row insert/read.

No benchmark files found. No Rust tests for metadata row creation by an indexer, exclusions, reconciliation, watcher updates, fuzzy ranking, or large-directory performance. No Angular tests found for search/indexing UI.

## Confidence

High: current repository has schema and DTOs but no implemented local metadata indexer/search command/watcher; directory listing is direct full filesystem scan. This is based on targeted grep for `metadata_index`, `SearchResult`, `WalkDir`, `fuzzy_match`, `ignore::`, and `notify::` under `src-tauri/src`.

Medium: bottleneck assessment is static; no runtime profiling was performed.

## Gaps / Open Questions

- Desired indexed roots are not defined in code (home dir only? mounted volumes? favorites? all accessible user folders?).
- No exclusion list exists yet; PRD says default system/app/dependency/cache exclusions are needed.
- No cloud placeholder detection implementation beyond `CloudState::Local` in file entries.
- No concurrency/backpressure policy for indexing vs user browsing/thumbnail work.
- No decision whether to use SQLite FTS5 vs simple indexed `LIKE` + Rust fuzzy scoring.

## Recommended Next Steps

1. Implement a backend indexing module first, not inside `commands.rs`: define roots, exclusions, metadata extraction, batched SQLite upserts, checkpoint/state transitions.
2. Use `walkdir`/`ignore` with `same_file_system`/symlink-loop safety and skip hidden/noisy dirs for indexing only.
3. Batch writes in transactions; update `index_state` with item counts instead of counting all rows on every bootstrap.
4. Add search command and frontend API after initial indexer exists; consider SQLite FTS5 or two-phase query (SQL prefilter then `fuzzy-matcher`).
5. Add tests for exclusion matching, metadata row creation, state transitions, pruning deleted paths, and search ranking.
6. For current browsing performance, consider caching lowercase sort keys, avoiding full metadata where current sort/view does not need it, and limiting thumbnail concurrency.

## Start Here

Open `src-tauri/src/commands.rs` around lines 457-568 first. It shows the current direct filesystem listing path and explains why current browsing work is O(all entries) even when pagination is requested. For actual indexing implementation, start with `src-tauri/src/persistence.rs` lines 142-219 for the existing schema and add a new backend module wired from `src-tauri/src/lib.rs` setup.
