<p align="center">
  <img src="app-logo.svg" width="120" alt="Frogger" />
</p>

<h1 align="center">Frogger</h1>

<p align="center">
  <strong>AI-native desktop file manager</strong><br/>
  Built with Tauri v2, React, and Rust
</p>

<p align="center">
  <a href="https://github.com/lilfourn/Frogger/stargazers">
    <img src="https://img.shields.io/github/stars/lilfourn/Frogger?style=flat&color=53ab41" alt="Stars" />
  </a>
  <a href="https://github.com/lilfourn/Frogger/issues">
    <img src="https://img.shields.io/github/issues/lilfourn/Frogger?style=flat&color=53ab41" alt="Issues" />
  </a>
  <a href="https://github.com/lilfourn/Frogger/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/lilfourn/Frogger?style=flat&color=53ab41" alt="License" />
  </a>
</p>

<p align="center">
  <a href="https://api.star-history.com/svg?repos=lilfourn/Frogger&type=Date">
    <img src="https://api.star-history.com/svg?repos=lilfourn/Frogger&type=Date" width="600" alt="Star History" />
  </a>
</p>

---

## What is Frogger?

A file manager that actually understands your files. Frogger combines a fast native file browser with local AI — semantic search, natural language commands, OCR, and Claude-powered file operations — all running on your machine.

## Features

**Core File Management**

- Four view modes: List, Grid, Column (Miller), Gallery
- Tabbed browsing with drag-and-drop between tabs
- Full undo/redo for all file operations (create, rename, move, copy, delete)
- Quick Look previews — images, code, markdown, PDF, video via `Space`
- Soft delete with dedicated trash and restore

**AI Intelligence** _(Phase 2–3, in progress)_

- Keyword-first search: exact filename/folder/path matches first, semantic fallback (vector embeddings) when keyword matches are empty
- OCR text extraction from images
- Claude-powered file operations in suggest or auto-execute mode
- Natural language commands: _"Move all PDFs to Documents"_
- Privacy-first: API key in OS keychain, per-directory permission scopes, full audit log

**Performance**

- Virtualized rendering for 10K+ file directories
- Rust backend for all filesystem operations
- SQLite with WAL mode for durable, fast data access
- Chunked copy with progress events and cancellation

## Tech Stack

| Layer           | Technology                                       |
| --------------- | ------------------------------------------------ |
| Desktop Runtime | Tauri v2                                         |
| Frontend        | React 19, TypeScript, Tailwind CSS 4             |
| State           | Zustand                                          |
| Backend         | Rust, SQLite (rusqlite), sqlite-vec              |
| AI              | Claude API (streaming), fastembed, Tesseract OCR |
| Testing         | Vitest, Playwright, cargo test                   |

## Quick Start

```bash
# prerequisites: Rust, Node.js 22+, pnpm
git clone https://github.com/lilfourn/Frogger.git
cd Frogger
pnpm install
pnpm tauri dev
```

## Development

```bash
pnpm typecheck        # type check
pnpm lint             # lint
pnpm test             # run unit tests (150+ tests)
cd src-tauri && cargo test  # run Rust tests
pnpm tauri dev        # launch dev build
```

## Architecture

```
src/                    # React frontend
├── components/         # UI components (layout, file-view, toolbar, sidebar, chat, quick-look)
├── hooks/              # Custom hooks (keyboard, drag-drop, file nav, quick look, events)
├── stores/             # Zustand stores (file, settings, nav)
├── services/           # Tauri IPC wrappers
└── types/              # Shared TypeScript types

src-tauri/src/          # Rust backend
├── commands/           # Tauri IPC command handlers
├── services/           # Business logic (file ops, undo, search, AI, OCR, indexing)
├── data/               # SQLite migrations + repository
├── models/             # Data structs (FileEntry, Operation, Volume)
└── shell/              # Shell execution + safety validation
```

## Roadmap

- [x] **Phase 0** — Project scaffolding
- [x] **Phase 1** — Core file manager (views, tabs, operations, undo/redo, quick look, keyboard nav)
- [ ] **Phase 2** — Local intelligence (search, indexing, OCR, embeddings)
- [ ] **Phase 3** — AI integration (Claude chat, suggest/auto-execute, onboarding)
- [ ] **Phase 4** — Polish & release (settings, smart folders, duplicates, E2E tests, CI/CD)

## License

MIT
