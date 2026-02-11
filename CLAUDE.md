# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Frogger is an AI-native desktop file manager built with Tauri v2 (React 19 frontend + Rust backend). Currently in Phase 1 (core file management). Phases 2-3 (AI/search/indexing) are not yet implemented.

## Commands

```bash
bun run dev              # Vite dev server only (no Tauri)
bun run tauri dev        # Full Tauri dev build (frontend + Rust)
bun run typecheck        # TypeScript type checking
bun run lint             # ESLint
bun run lint:fix         # ESLint with auto-fix
bun run test             # Vitest (all unit tests)
bun run test -- -t "name"  # Single test by name
bun run test:e2e         # Playwright e2e tests

cd src-tauri && cargo test    # Rust backend tests
cd src-tauri && cargo clippy -- -D warnings  # Rust lints
```

CI runs with pnpm (`pnpm-lock.yaml` is the lockfile). CI checks: typecheck, lint, vitest, cargo test, cargo clippy.

## Architecture

### Frontend → Backend Communication

All filesystem operations go through Tauri IPC. The call chain:

`React Component → fileService.ts (invoke()) → Rust file_commands.rs → file_service.rs/undo_service.rs → SQLite`

- `src/services/fileService.ts` — thin wrappers around `invoke<T>()` calls. Every Rust command has a 1:1 TypeScript wrapper here.
- `src-tauri/src/commands/file_commands.rs` — Tauri `#[command]` handlers. Registered in `lib.rs` via `invoke_handler`.
- `src-tauri/src/services/` — business logic: `file_service.rs` (filesystem ops), `undo_service.rs` (operation history).

### State Management

Two Zustand stores, no middleware:

- `fileStore` — current path, entries, tabs, selection, sorting. Tabs track independent paths. `sortedEntries()` is a derived getter (not state).
- `settingsStore` — theme, view mode, sidebar, hidden files.

Stores are accessed via selectors (`useFileStore(s => s.field)`) to minimize re-renders.

### Rust Backend State

`AppState` (managed by Tauri) holds:
- `db: Mutex<Connection>` — single SQLite connection (WAL mode)
- `cancel_flag: Arc<AtomicBool>` — for cancelling long copy operations

Database: `frogger.db` in Tauri's app data dir. Migrations in `src-tauri/src/data/migrations.rs`. Repository pattern in `data/repository.rs`.

### Component Structure

`App.tsx` orchestrates: loads entries on path change, wires keyboard shortcuts, composes layout.

Layout: `AppLayout` (sidebar + main) → main contains `TabBar` + `Toolbar` + `FileView`. `QuickLookPanel` is a sibling overlay.

View modes (list, grid, column, gallery) are in `src/components/file-view/`.

### Testing

Tests are colocated with source files (`*.test.ts` / `*.test.tsx`). Vitest with jsdom environment. Setup file imports `@testing-library/jest-dom/vitest`.

Tauri `invoke` calls must be mocked in tests (`vi.mock("@tauri-apps/api/core")`).

### Key Patterns

- Keyboard shortcuts defined as config arrays in `App.tsx`, handled by `useKeyboardShortcuts` hook
- File operations (undo/redo/delete/rename/createDir) encapsulated in `useFileOperations` hook
- Quick Look via `useQuickLook` hook, toggled by Space key on selected file
- Shell command execution in Rust has safety validation (`src-tauri/src/shell/safety.rs`)
