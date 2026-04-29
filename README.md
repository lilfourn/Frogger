# Frogger

Frogger is a Tauri v2 + Angular desktop file manager. Phase 1 replaces the starter app with a Finder-style shell, persisted workspace restoration, metadata indexing, global fuzzy search, previews, settings, and safe core file operations.

## Development Commands

- `bun run start` — run the Angular dev server.
- `bun run tauri dev` — run Frogger in Tauri development mode.
- `bun run build` — build the Angular frontend for production.
- `bun run typecheck` — run TypeScript type checking.
- `bun run test:frontend` — run the current frontend smoke build until Angular component tests are added.
- `bun run test:rust` — run Rust unit tests in `src-tauri`.
- `bun run test` — run frontend smoke build and Rust tests.
- `bun run check` — run production frontend build and Rust tests.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) + [Angular Language Service](https://marketplace.visualstudio.com/items?itemName=Angular.ng-template).
