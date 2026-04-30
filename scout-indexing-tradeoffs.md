# Code Context

## Files Retrieved
1. `src-tauri/src/persistence.rs` (lines 1-220) - DB open/configuration, metadata index schema, index state schema, default settings.
2. `src-tauri/src/commands.rs` (lines 1-83) - bootstrap entry point currently loads indexing state only.
3. `src-tauri/src/commands.rs` (lines 473-689) - existing directory metadata extraction flow that indexing can reuse or deliberately avoid coupling to browsing.
4. `src-tauri/src/commands.rs` (lines 760-789) - existing thumbnail cache key hashing pattern using path/size/mtime.
5. `src-tauri/src/commands.rs` (lines 1191-1217) - index state loader and current count query.
6. `src-tauri/src/commands.rs` (lines 1898-1922) - only Rust indexing-related test: state/count read from manually inserted row.
7. `src-tauri/src/lib.rs` (lines 1-55) - registered Tauri commands; no indexing/search commands registered.
8. `src-tauri/Cargo.toml` (lines 19-37) - relevant dependencies already present: `ignore`, `notify`, `rusqlite`, `tokio`, `walkdir`, `fuzzy-matcher`.
9. `src-tauri/src/models.rs` (lines 274-315) - `SearchResult`, `IndexingState`, and statuses.
10. `src/app/core/frogger-api.types.ts` (lines 1-64, 192-209, 245-247) - frontend API contracts for indexing/search/events/settings.
11. `src/app/core/frogger-api.service.ts` (lines 1-73) - frontend invokes existing commands; no search/indexing API yet.
12. `src/app/app.component.ts` (lines 596-604) and `src/app/app.component.html` (lines 177-180, 413-418) - search disabled/status UI is driven only by `hasInitialIndex`.
13. `docs/tasks-prd-frogger-phase-1-initial-file-manager.md` (lines 476-531, 1000-1005) - product requirements for metadata indexing, exclusions, watchers, reconciliation, and SQLite concurrency risks.

## Key Code

### Current state: schema and UI exist, indexing engine does not
- `src-tauri/src/persistence.rs` lines 142-164 defines `metadata_index` with columns needed for metadata-only indexing: `path`, `parent_path`, `name`, `display_name`, `kind`, `is_dir`, `size`, `modified_at`, `created_at`, `hidden`, `extension`, `search_text`, `recent_boost`, `modified_boost`, plus indexes on parent/name/kind/is_dir/search_text.
- `src-tauri/src/persistence.rs` lines 166-174 defines `index_state` with `status`, `has_initial_index`, timestamps, `checkpoint_json`, and `error_json`.
- `src-tauri/src/persistence.rs` lines 15-23 configures SQLite with WAL, `synchronous=NORMAL`, and `busy_timeout=5000`, which is favorable for background indexing plus UI reads.
- `src-tauri/src/lib.rs` lines 40-55 registers commands; there is no indexing/search command.
- `src-tauri/src/commands.rs` lines 1191-1217 only reads state and `COUNT(*)`; it does not start or update indexing.
- `src/app/core/frogger-api.service.ts` lines 6-73 has no methods for starting indexing, reading progress, or search.
- `src/app/app.component.ts` lines 596-604 and `src/app/app.component.html` lines 177-180 disable search until `state.indexing.hasInitialIndex` is true.

### Existing reusable metadata extraction
`src-tauri/src/commands.rs` lines 473-689 implements directory listing by:
- `metadata()` on target folder, then `read_dir()`.
- Per-entry `symlink_metadata()` first.
- Falls back for broken symlinks.
- Computes `is_dir`, size, modified/created times, hidden, extension, display name, kind, readonly, icon, symlink fields.

This is the closest local pattern for indexing rows, but indexing should probably use a slimmer helper returning DB row fields rather than full UI `FileEntry`/icon data.

### Existing hash/cache pattern
`src-tauri/src/commands.rs` lines 779-789 hashes `source_path`, `source_size`, and `source_modified_at` to name thumbnail cache files. For indexing, the practical equivalent is not content hashing; use metadata fingerprints such as `(path, size, modified_at, is_dir)` stored in `metadata_index`/checkpoint data. Full content hashes would violate metadata-only speed/privacy goals.

### Dependencies already available
`src-tauri/Cargo.toml` lines 19-37 already includes likely building blocks:
- `ignore` for `.gitignore`-style/exclusion-aware walking.
- `walkdir` for traversal.
- `notify` for watchers.
- `tokio` with multi-thread runtime.
- `rusqlite` for DB writes.
- `fuzzy-matcher` for later search ranking.

## Architecture

Bootstrap currently opens the app DB, loads settings, detects home access, restores windows, loads sidebar, then loads indexing state (`src-tauri/src/commands.rs` lines 43-83). The frontend receives `AppBootstrap.indexing` and uses only `hasInitialIndex` to disable/enable search and status. There is no background worker, event emission, indexing command, search command, watcher, or reconciliation loop yet.

The intended architecture from docs is: after filesystem permission approval, run a fast metadata-only pass, persist rows/checkpoints in SQLite, emit minimal progress via `frogger://indexing-progress`, enable search after any initial index exists, then keep fresh through exclusions, watchers, startup reconciliation, and pruning (`docs/tasks...` lines 476-531). The docs also call out SQLite concurrency as a risk (`docs/tasks...` lines 1000-1005).

## Tradeoff Assessment

### 1. Incremental indexing / reconciliation
**Fit:** Highest long-term impact. Schema already has `index_state.checkpoint_json` and per-row timestamps (`src-tauri/src/persistence.rs` lines 142-174). Docs explicitly require checkpoints, watchers, pruning, and changed/new-path updates.

**Practical approach:**
- First implement full initial pass over allowed roots.
- Store checkpoint info: roots, started/completed time, excluded pattern version, maybe last completed root/path.
- For later launches, scan directory metadata and compare `(path, modified_at, size, is_dir)` against existing rows; upsert changed/new; prune paths no longer present.
- Add watcher-fed queue after baseline works.

**Risks:**
- Directory mtimes are not a reliable complete signal for deep tree changes on all platforms; full reconciliation still needed occasionally.
- Renames look like delete+insert unless inode/file IDs are added later.
- Pruning by root can be expensive without batching.

**Speed tradeoff:** Great after first run; less helpful for cold clean profile.

### 2. Caching/hashing
**Fit:** Good if limited to metadata fingerprints. Existing thumbnail code hashes path/size/mtime for cache identity (`src-tauri/src/commands.rs` lines 779-789), and metadata rows already carry size/modified fields.

**Recommended:** Use metadata fingerprints, not content hashes. Add/derive a stable `fingerprint` or compare existing columns. Avoid reading file contents to preserve metadata-only/cloud-safe requirements.

**Risks:**
- mtime resolution and provider behavior can miss changes.
- Hashing every path string is cheap, but adding a new DB column requires migration.

**Speed tradeoff:** Reduces DB writes/parsing on reconciliation. Does not reduce traversal cost unless paired with directory-level checkpoints/exclusions.

### 3. Parallel file reads/parsing
**Fit:** Moderate, with caution. The work is mostly filesystem metadata calls and path processing. `tokio` multi-thread is available, but `rusqlite::Connection` is not a shared async pool.

**Recommended:**
- Parallelize traversal/metadata extraction with a bounded worker pool or `spawn_blocking` tasks.
- Send normalized row structs over a bounded channel to one DB writer thread/task.
- Cap concurrency to avoid thrashing slow disks/cloud folders and starving visible browsing.

**Risks:**
- Unbounded parallel `metadata()` can make the app feel slower and trigger cloud provider/network churn.
- SQLite writes from many threads can hit busy timeouts despite WAL.
- Current browser uses synchronous `read_dir`/metadata in command handlers (`src-tauri/src/commands.rs` lines 473-689); indexing must not block those handlers.

**Speed tradeoff:** Useful for large SSD trees; potentially harmful on network/cloud/slow disks without backpressure.

### 4. Batching database writes
**Fit:** Very high and probably the easiest big win. Current DB config already supports WAL (`src-tauri/src/persistence.rs` lines 15-23), and schema uses a primary-key path suitable for upsert.

**Recommended:**
- Accumulate rows (e.g. 250-2,000 rows) and write in one transaction using a prepared `INSERT ... ON CONFLICT(path) DO UPDATE`.
- Update `index_state` less frequently (time-based or batch-based), not per file.
- Consider temporarily deferring secondary index maintenance only for a first-ever bulk build if implementation complexity is acceptable; otherwise keep simple batched transactions first.

**Risks:**
- Large transactions delay progress visibility and increase rollback cost.
- Small transactions cause fsync/lock overhead; tune batch size.
- UI reads/search during writes require careful busy handling; WAL helps but not all lock scenarios vanish.

**Speed tradeoff:** Likely best speed/complexity ratio.

### 5. Excluding patterns
**Fit:** Very high. Docs require excluding system/app/program internals, dependency dirs, VCS, build outputs, venvs, package caches, while keeping browsing truthful (`docs/tasks...` lines 504-531). `ignore` dependency is already present.

**Recommended:**
- Use `ignore::WalkBuilder` rather than raw `walkdir` for default ignore/gitignore behavior plus custom overrides.
- Add default skip names such as `.git`, `node_modules`, `target`, `dist`, `.angular`, `.cocoindex_code`, `Library/Caches`, package caches, venv dirs.
- Keep exclusion logic in backend indexing module only; do not reuse it in `list_directory_impl`, because docs require excluded folders remain visible while browsing.

**Risks:**
- Over-exclusion makes search feel incomplete.
- Under-exclusion explodes index size and slows first run.
- Pattern semantics across absolute roots can be tricky; needs tests.

**Speed tradeoff:** Extremely high on developer machines; this repository itself contains `node_modules`, `dist`, `.angular`, `.git`, and `.cocoindex_code` directories that should not be indexed.

### 6. Streaming/backpressure
**Fit:** High because indexing is background work in an interactive file manager. Event name exists in model/API contracts (`src-tauri/src/models.rs` lines 394-406; frontend types lines 245-247), but no emitter/consumer is wired.

**Recommended:**
- Pipeline: walker -> bounded metadata workers -> bounded DB writer.
- Use bounded channels to keep memory flat and throttle metadata calls when DB falls behind.
- Emit progress every N rows or every T ms, not per entry.
- Add cancellation/pause priority hooks so user operations outrank indexing, matching docs.

**Risks:**
- Progress totals are hard without pre-walking; avoid expensive total counts.
- Cancellation can leave partial state; use `index_state.status` and checkpoints to recover.

**Speed tradeoff:** May not maximize raw throughput, but improves perceived speed and UI responsiveness.

### 7. Benchmark instrumentation
**Fit:** Essential before optimizing too far. There is no indexing benchmark or implementation yet; easiest is to instrument from day one.

**Recommended counters:**
- roots scanned, dirs visited, files visited, skipped dirs/files, metadata errors.
- rows inserted/updated/unchanged/deleted.
- DB batch sizes and batch duration.
- queue depths/backpressure wait time.
- elapsed time to `has_initial_index=true`.

**Validation path:**
- Add a Rust test helper/temp tree and a non-Tauri indexing function benchmark-ish unit/integration test first.
- Manual generated tree: nested dirs + many files + excluded `node_modules`/`target` + symlinks + deleted/changed files.
- Compare single-thread unbatched vs batched single writer vs bounded parallel metadata extraction.

**Risks:**
- Logging too much per file will itself slow indexing.
- Tauri event emission frequency can dominate if too chatty.

## Likely Implementation Priority

1. **Create a backend indexing module** (`src-tauri/src/indexing.rs` or similar) with pure functions for exclusion matching, row extraction, batching/upsert, and state transitions. Register startup spawning from `bootstrap_app`/setup only after permission is granted.
2. **Batch DB writes** with transactions and prepared upsert. This is the easiest performance win.
3. **Use `ignore::WalkBuilder` plus default exclusions** before scanning user roots. This prevents avoidable work.
4. **Metadata fingerprint incremental reconciliation** using existing columns/checkpoint JSON. Avoid content hashing.
5. **Bounded streaming pipeline** with one DB writer and bounded metadata concurrency. Start single-threaded walker + writer first, then add worker pool if benchmarks show metadata reads dominate.
6. **Watcher queue via `notify`** after baseline indexing/reconciliation is correct.
7. **Instrumentation and tests** alongside each stage.

## Easiest Validation Path

- Write backend-only tests against temp SQLite DB and temp dirs; do not start with Angular/Tauri UI.
- Validate that `metadata_index` rows are created, `index_state` transitions from `not_started` -> `initial_build` -> `ready`, and `has_initial_index` flips after first successful batch/pass.
- Validate excluded directories are skipped by indexing but still visible through existing `list_directory`.
- Validate reconciliation updates changed/new rows and prunes deleted paths.
- Add a debug/manual command or log summary for elapsed time and rows/sec on generated trees.

## Risks and Constraints

- **No current indexing engine:** Most options are architectural choices, not tweaks to existing code.
- **SQLite concurrency:** WAL is enabled, but multiple writers or unbounded event/search reads can still contend.
- **Cloud safety:** Current `FileEntry.cloud` is always `CloudState::Local` in directory listing; cloud placeholder detection is not implemented. Indexing must avoid content reads and should centralize safe-read policy before thumbnails/previews expand.
- **Symlinks:** Current listing handles broken symlinks carefully. Indexing should avoid following symlink loops unless explicitly desired.
- **UI contract:** Search is disabled solely by `hasInitialIndex`; partial first-pass semantics must be clear.
- **Schema migration:** Current schema version is 1. Adding fingerprint columns, FTS, or roots tables requires migration work.

## Confidence

Medium-high that batching writes + exclusions + metadata-only incremental reconciliation are the best first tradeoffs, because they align with existing schema/dependencies and product requirements. Medium on parallelism gains because actual bottlenecks need measurement on target machines/cloud folders.

## Gaps / Open Questions

- Which roots should initial indexing cover: home only, sidebar locations, favorites, mounted drives, or user-configured locations?
- Should search use plain indexed `search_text`, SQLite FTS5, or application-level fuzzy ranking over candidate rows?
- How soon should `has_initial_index` become true: after any rows exist, after all configured roots complete, or after a minimum usable subset?
- What cloud providers must be detected on each OS, and what metadata calls are guaranteed not to hydrate cloud-only files?
- What priority/cancellation mechanism should file operations use to pause indexing?

## Start Here

Start with `src-tauri/src/persistence.rs` lines 142-174 and `src-tauri/src/commands.rs` lines 1191-1217. They define the existing index storage/state contract, and they make clear that the missing piece is a backend indexing worker that writes `metadata_index` and transitions `index_state`.
