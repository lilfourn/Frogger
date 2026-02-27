# Frogger 🐸 — Spec Requirements

## Overview

This document defines the functional requirements, non-functional requirements, and acceptance criteria for every feature of the Frogger file manager.

---

## 1. Core File Manager Requirements

### 1.1 Navigation & Layout

| ID     | Requirement                              | Priority | Acceptance Criteria                                                     |
| ------ | ---------------------------------------- | -------- | ----------------------------------------------------------------------- |
| NAV-01 | Sidebar with bookmarked folders          | P0       | Users can pin/unpin any folder to sidebar; persists across sessions     |
| NAV-02 | Recent locations list                    | P0       | Last 20 visited directories displayed; click to navigate                |
| NAV-03 | Connected drives detection               | P0       | Auto-detect mounted volumes on all three platforms                      |
| NAV-04 | Smart folders (by type, date, AI)        | P1       | Auto-populated virtual folders; user can create custom rules            |
| NAV-05 | Tabbed browsing                          | P0       | Cmd/Ctrl+T opens new tab; drag tabs to reorder; close with middle-click |
| NAV-06 | View modes (grid, list, column, gallery) | P0       | Toggle via toolbar; preference saved per-directory                      |
| NAV-07 | Breadcrumb path bar                      | P0       | Each segment clickable; supports editable text input mode               |
| NAV-08 | Drag-and-drop                            | P0       | Files can be dragged between tabs, sidebar, and within views            |
| NAV-09 | Keyboard navigation                      | P0       | Arrow keys, Enter to open, Backspace to go up, Tab to cycle panels      |

### 1.2 Quick Look Previews

| ID     | Requirement           | Priority | Acceptance Criteria                                        |
| ------ | --------------------- | -------- | ---------------------------------------------------------- |
| QLK-01 | Trigger via Space key | P0       | Configurable shortcut; toggles preview on/off              |
| QLK-02 | Image preview         | P0       | Supports JPEG, PNG, GIF, WebP, SVG; zoom with scroll wheel |
| QLK-03 | Video preview         | P0       | Supports MP4, WebM, MOV; play/pause/seek controls          |
| QLK-04 | PDF preview           | P0       | Scrollable rendered pages; page navigation; search within  |
| QLK-05 | Code file preview     | P0       | Syntax highlighting with language auto-detection           |
| QLK-06 | Markdown preview      | P0       | Rendered with proper heading, list, code block formatting  |
| QLK-07 | Audio preview         | P1       | Waveform visualization; play/pause/seek; metadata display  |
| QLK-08 | Metadata overlay      | P1       | Show file size, dimensions, duration, EXIF on preview      |
| QLK-09 | OCR text overlay      | P2       | Selectable text overlay on images with OCR data available  |

### 1.3 File Operations

| ID     | Requirement                 | Priority | Acceptance Criteria                                              |
| ------ | --------------------------- | -------- | ---------------------------------------------------------------- |
| FOP-01 | Create files and folders    | P0       | Right-click → New; keyboard shortcut; inline rename on create    |
| FOP-02 | Rename (inline edit)        | P0       | Click-to-rename or F2; validates filename for OS rules           |
| FOP-03 | Move files                  | P0       | Drag-drop or cut/paste; shows progress for large moves           |
| FOP-04 | Copy files                  | P0       | Cmd/Ctrl+C, Cmd/Ctrl+V; progress indicator; handles duplicates   |
| FOP-05 | Delete files                | P0       | Moves to Frogger trash (soft delete); Cmd/Ctrl+Delete            |
| FOP-06 | Batch selection             | P0       | Shift+click for range; Cmd/Ctrl+click for individual; Select All |
| FOP-07 | Bulk operations             | P0       | All ops work on multi-selection; progress bar for batch          |
| FOP-08 | File info panel             | P0       | Right-click → Info; shows size, dates, permissions, type         |
| FOP-09 | Undo/redo                   | P0       | Cmd/Ctrl+Z/Shift+Z; works for move, rename, delete, copy         |
| FOP-10 | Platform keyboard shortcuts | P0       | Mirrors native conventions per OS                                |

### 1.4 Theme & Appearance

| ID     | Requirement                | Priority | Acceptance Criteria                                       |
| ------ | -------------------------- | -------- | --------------------------------------------------------- |
| THM-01 | Light mode                 | P0       | Clean, minimal design; proper contrast ratios (WCAG AA)   |
| THM-02 | Dark mode                  | P0       | True dark theme; no white flashes on transition           |
| THM-03 | System auto-detect         | P0       | Follows OS light/dark preference; updates in real-time    |
| THM-04 | macOS-inspired aesthetics  | P0       | Translucent sidebars, rounded corners, subtle shadows     |
| THM-05 | Platform-adaptive elements | P1       | Window controls match OS; scrollbar style adapts          |
| THM-06 | Smooth transitions         | P1       | Framer Motion animations for view changes, panel toggling |

---

## 2. AI Features Requirements

### 2.1 Chat Sidebar

| ID     | Requirement               | Priority | Acceptance Criteria                                             |
| ------ | ------------------------- | -------- | --------------------------------------------------------------- |
| AIS-01 | Collapsible sidebar panel | P0       | Toggle with keyboard shortcut; animates open/close              |
| AIS-02 | Natural language input    | P0       | Free-text input; supports multi-line; Enter to send             |
| AIS-03 | Context-aware responses   | P0       | Claude receives current directory, selected files, view state   |
| AIS-04 | Persistent chat history   | P0       | Conversations saved to SQLite; searchable; deletable            |
| AIS-05 | Streaming responses       | P0       | Tokens stream in real-time via SSE; cancel button available     |
| AIS-06 | Suggest mode (default)    | P0       | Shows diff preview; user approves/rejects before execution      |
| AIS-07 | Auto-execute mode         | P1       | Executes immediately; full undo history; toggleable             |
| AIS-08 | Operation diff preview    | P0       | Visual diff showing source → destination, old name → new name   |
| AIS-09 | Error handling            | P0       | Graceful API errors; rate limit messages; network offline state |

### 2.2 AI Commands

| ID     | Requirement                     | Priority | Acceptance Criteria                                            |
| ------ | ------------------------------- | -------- | -------------------------------------------------------------- |
| AIC-01 | Move/copy/delete by description | P0       | "Move all screenshots to new folder" executes correctly        |
| AIC-02 | Batch rename with patterns      | P0       | EXIF-based, prefix/suffix, regex patterns via natural language |
| AIC-03 | Document summarization          | P0       | "What's this PDF about?" returns concise summary               |
| AIC-04 | Natural language file search    | P0       | "Find the tax doc from last month" returns relevant results    |
| AIC-05 | Project structure creation      | P1       | "Set up a React project" creates appropriate folder tree       |
| AIC-06 | Content-based categorization    | P1       | Reads file contents to assign categories beyond extension      |
| AIC-07 | Project detection               | P1       | Identifies Node.js, Python, LaTeX, etc. project structures     |
| AIC-08 | Duplicate detection             | P1       | Hash-based exact + content similarity for near-duplicates      |
| AIC-09 | Large/old file surfacing        | P2       | Generates cleanup reports; identifies temp/cache files         |
| AIC-10 | Downloads folder organization   | P1       | "Organize Downloads by file type" creates sorted structure     |

---

## 3. OCR Requirements

| ID     | Requirement                   | Priority | Acceptance Criteria                                       |
| ------ | ----------------------------- | -------- | --------------------------------------------------------- |
| OCR-01 | Automatic OCR on current view | P0       | Processes images/PDFs when directory is navigated to      |
| OCR-02 | Local processing (no API)     | P0       | Uses Tesseract via Rust bindings; zero network calls      |
| OCR-03 | Cached results                | P0       | Files processed once; re-processed only if modified       |
| OCR-04 | Background processing         | P0       | Non-blocking; bounded concurrency (max 4 threads)         |
| OCR-05 | OCR availability indicator    | P1       | Subtle badge/icon on file thumbnails when OCR text exists |
| OCR-06 | Multi-language support        | P2       | Configurable Tesseract language packs                     |
| OCR-07 | Search integration            | P0       | OCR text indexed in FTS5 and vector store                 |
| OCR-08 | Per-directory toggle          | P1       | Can disable OCR for specific directories                  |

---

## 4. Semantic Search Requirements

| ID     | Requirement                          | Priority | Acceptance Criteria                                                                                                               |
| ------ | ------------------------------------ | -------- | --------------------------------------------------------------------------------------------------------------------------------- |
| SRC-01 | Universal search bar                 | P0       | Cmd/Ctrl+F or Cmd/Ctrl+P; always accessible                                                                                       |
| SRC-02 | Keyword search (FTS5)                | P0       | Matches filenames, paths, OCR text; ranked results                                                                                |
| SRC-03 | Semantic vector search               | P0       | Natural language queries return semantically relevant files                                                                       |
| SRC-04 | Keyword-first with semantic fallback | P0       | Returns exact/prefix keyword matches first; falls back to vector search only when keyword results are empty and query length >= 2 |
| SRC-05 | Local embedding generation           | P0       | `all-MiniLM-L6-v2` via fastembed-rs; no network required                                                                          |
| SRC-06 | Incremental indexing                 | P0       | File watcher triggers re-index on changes; no full re-scans                                                                       |
| SRC-07 | Scope controls                       | P0       | Search current directory, subtree, or entire indexed system                                                                       |
| SRC-08 | Filters and facets                   | P1       | Filter by type, date range, size, AI category                                                                                     |
| SRC-09 | Deep search via Claude               | P2       | Optional API call for ambiguous queries requiring reasoning                                                                       |
| SRC-10 | Instant results                      | P0       | Results appear as user types; debounced at 150ms                                                                                  |

---

## 5. Onboarding & Permissions Requirements

| ID     | Requirement                  | Priority | Acceptance Criteria                                                |
| ------ | ---------------------------- | -------- | ------------------------------------------------------------------ |
| ONB-01 | First-run welcome screen     | P0       | Explains app purpose; skippable but not dismissible accidentally   |
| ONB-02 | API key setup                | P0       | Input field; validates key against Anthropic API; shows model info |
| ONB-03 | Secure key storage           | P0       | Key stored in OS keychain; never written to disk as plaintext      |
| ONB-04 | File content scan permission | P0       | Global, per-directory, or off; clearly explained                   |
| ONB-05 | File modification permission | P0       | Toggle: Claude can execute vs. only suggest                        |
| ONB-06 | OCR permission toggle        | P0       | Enable/disable automatic OCR                                       |
| ONB-07 | Indexing scope selection     | P0       | User selects which directories to index                            |
| ONB-08 | Privacy summary              | P0       | Plain-English summary: what leaves device vs. what stays local     |
| ONB-09 | Privacy audit log            | P1       | Every API call logged with timestamp, data sent, token count       |
| ONB-10 | Per-directory overrides      | P1       | Exclude specific directories from all AI/OCR/indexing              |
| ONB-11 | Index clear option           | P1       | One-click purge of all local index data                            |
| ONB-12 | Skip AI setup                | P0       | Core file manager fully functional without API key                 |

---

## 6. Non-Functional Requirements

### 6.1 Performance

| ID     | Requirement                   | Target                      |
| ------ | ----------------------------- | --------------------------- |
| PER-01 | Cold start time               | < 1.5 seconds               |
| PER-02 | Directory listing (1K files)  | < 200ms                     |
| PER-03 | Directory listing (10K files) | < 800ms with virtualization |
| PER-04 | Search response (local)       | < 100ms                     |
| PER-05 | Embedding generation          | < 50ms per file             |
| PER-06 | OCR processing                | < 2s per single-page image  |
| PER-07 | Idle memory usage             | < 120MB                     |
| PER-08 | Bundle size                   | < 15MB per platform         |
| PER-09 | UI frame rate                 | 60fps for animations        |

### 6.2 Reliability

| ID     | Requirement          | Target                                                                |
| ------ | -------------------- | --------------------------------------------------------------------- |
| REL-01 | Crash recovery       | Auto-save state every 30s; restore tabs/path on restart               |
| REL-02 | Graceful degradation | If Claude API is down, all non-AI features work normally              |
| REL-03 | Data integrity       | Undo log survives crashes; SQLite WAL mode for durability             |
| REL-04 | Error boundaries     | React error boundaries prevent full-app crashes from component errors |
| REL-05 | Offline mode         | Full file management + local search works without internet            |

### 6.3 Security

| ID     | Requirement                 | Details                                                      |
| ------ | --------------------------- | ------------------------------------------------------------ |
| SEC-01 | API key encryption          | Stored in OS-level keychain; never in config files           |
| SEC-02 | Shell injection prevention  | Parameterized command execution; no raw string interpolation |
| SEC-03 | Scoped file access          | Tauri capability scopes enforce directory boundaries         |
| SEC-04 | Audit logging               | Every API call logged with data sent and tokens used         |
| SEC-05 | CSP enforcement             | Strict Content Security Policy in WebView                    |
| SEC-06 | No telemetry                | Zero analytics or tracking; fully offline-capable            |
| SEC-07 | Destructive op confirmation | Delete/overwrite always requires explicit user approval      |

### 6.4 Accessibility

| ID     | Requirement           | Details                                                |
| ------ | --------------------- | ------------------------------------------------------ |
| ACC-01 | Keyboard navigation   | Full app usable without mouse                          |
| ACC-02 | Screen reader support | ARIA labels on all interactive elements via React Aria |
| ACC-03 | Color contrast        | WCAG AA minimum (4.5:1 for text) in both themes        |
| ACC-04 | Focus indicators      | Visible focus rings on all focusable elements          |
| ACC-05 | Reduced motion        | Respects `prefers-reduced-motion` OS setting           |

### 6.5 Testing Strategy

| Level             | Tool                     | Coverage Target                                              |
| ----------------- | ------------------------ | ------------------------------------------------------------ |
| Unit (Rust)       | `cargo test`             | All service layer functions, command pattern, search ranking |
| Unit (React)      | Vitest + Testing Library | All components, stores, hooks                                |
| Integration       | Tauri test driver        | IPC round-trips, file operations, undo/redo sequences        |
| E2E               | Playwright               | Critical user flows on all 3 platforms                       |
| Visual Regression | Playwright screenshots   | Component appearance across themes and platforms             |
| Performance       | Custom benchmarks        | Directory listing, search, OCR timing                        |

---

## 7. Project Structure

```
frogger/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── state.rs                # AppState definition
│   │   ├── error.rs                # Error types (thiserror)
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── file_commands.rs
│   │   │   ├── ai_commands.rs
│   │   │   ├── search_commands.rs
│   │   │   ├── ocr_commands.rs
│   │   │   └── perm_commands.rs
│   │   ├── services/
│   │   │   ├── mod.rs
│   │   │   ├── file_service.rs
│   │   │   ├── ai_service.rs
│   │   │   ├── search_service.rs
│   │   │   ├── ocr_service.rs
│   │   │   ├── undo_service.rs
│   │   │   ├── permission_service.rs
│   │   │   └── embedding_service.rs
│   │   ├── data/
│   │   │   ├── mod.rs
│   │   │   ├── repository.rs       # SQLite CRUD operations
│   │   │   ├── vector_store.rs     # sqlite-vec interface
│   │   │   ├── migrations.rs       # Schema migrations
│   │   │   └── keyring.rs          # OS keychain wrapper
│   │   ├── shell/
│   │   │   ├── mod.rs
│   │   │   ├── executor.rs         # Platform-adaptive shell execution
│   │   │   ├── safety.rs           # Command validation & sanitization
│   │   │   └── commands.rs         # Unix/PowerShell command builders
│   │   └── models/
│   │       ├── mod.rs
│   │       ├── file_entry.rs
│   │       ├── operation.rs
│   │       ├── search_result.rs
│   │       ├── chat_message.rs
│   │       └── permissions.rs
│   └── tests/
│       ├── file_operations.rs
│       ├── search_tests.rs
│       ├── undo_tests.rs
│       └── ai_service_tests.rs
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppLayout.tsx
│   │   │   ├── TitleBar.tsx
│   │   │   └── StatusBar.tsx
│   │   ├── sidebar/
│   │   │   ├── Sidebar.tsx
│   │   │   ├── NavTree.tsx
│   │   │   ├── Favorites.tsx
│   │   │   ├── DrivesList.tsx
│   │   │   └── SmartFolders.tsx
│   │   ├── file-view/
│   │   │   ├── FileView.tsx         # Polymorphic view container
│   │   │   ├── GridView.tsx
│   │   │   ├── ListView.tsx
│   │   │   ├── ColumnView.tsx
│   │   │   ├── GalleryView.tsx
│   │   │   ├── FileItem.tsx
│   │   │   └── Breadcrumb.tsx
│   │   ├── quick-look/
│   │   │   ├── QuickLookPanel.tsx
│   │   │   ├── ImagePreview.tsx
│   │   │   ├── VideoPreview.tsx
│   │   │   ├── PdfPreview.tsx
│   │   │   ├── CodePreview.tsx
│   │   │   ├── MarkdownPreview.tsx
│   │   │   └── AudioPreview.tsx
│   │   ├── chat/
│   │   │   ├── ChatSidebar.tsx
│   │   │   ├── ChatInput.tsx
│   │   │   ├── MessageList.tsx
│   │   │   ├── MessageBubble.tsx
│   │   │   ├── DiffPreview.tsx
│   │   │   └── ApprovalBar.tsx
│   │   ├── toolbar/
│   │   │   ├── Toolbar.tsx
│   │   │   ├── ViewToggle.tsx
│   │   │   ├── SortDropdown.tsx
│   │   │   └── SearchBar.tsx
│   │   ├── onboarding/
│   │   │   ├── OnboardingWizard.tsx
│   │   │   ├── WelcomeStep.tsx
│   │   │   ├── ApiKeyStep.tsx
│   │   │   ├── PermissionStep.tsx
│   │   │   └── PrivacySummaryStep.tsx
│   │   ├── settings/
│   │   │   ├── SettingsModal.tsx
│   │   │   ├── GeneralSettings.tsx
│   │   │   ├── AppearanceSettings.tsx
│   │   │   ├── PermissionSettings.tsx
│   │   │   └── PrivacyLogViewer.tsx
│   │   └── shared/
│   │       ├── ContextMenu.tsx
│   │       ├── Modal.tsx
│   │       ├── ProgressBar.tsx
│   │       ├── Tooltip.tsx
│   │       └── ErrorBoundary.tsx
│   ├── hooks/
│   │   ├── useFileOperations.ts
│   │   ├── useKeyboardShortcuts.ts
│   │   ├── useDragAndDrop.ts
│   │   ├── useQuickLook.ts
│   │   ├── useSearch.ts
│   │   └── useTauriEvents.ts
│   ├── stores/
│   │   ├── fileStore.ts
│   │   ├── chatStore.ts
│   │   ├── undoStore.ts
│   │   ├── settingsStore.ts
│   │   └── searchStore.ts
│   ├── services/
│   │   ├── fileService.ts           # invoke() wrappers for file ops
│   │   ├── aiService.ts             # invoke() wrappers for AI ops
│   │   ├── searchService.ts
│   │   └── settingsService.ts
│   ├── types/
│   │   ├── file.ts
│   │   ├── chat.ts
│   │   ├── search.ts
│   │   └── settings.ts
│   └── styles/
│       ├── globals.css
│       └── themes/
│           ├── light.css
│           └── dark.css
├── tests/
│   ├── e2e/
│   │   ├── navigation.spec.ts
│   │   ├── file-operations.spec.ts
│   │   ├── search.spec.ts
│   │   └── chat.spec.ts
│   └── components/
│       ├── FileView.test.tsx
│       ├── ChatSidebar.test.tsx
│       └── Breadcrumb.test.tsx
├── .github/
│   └── workflows/
│       ├── ci.yml                   # Lint + test on PR
│       ├── build.yml                # Cross-platform build
│       └── release.yml              # Tagged release with signing
├── package.json
├── pnpm-lock.yaml
├── tsconfig.json
├── tailwind.config.ts
├── vite.config.ts
├── vitest.config.ts
├── playwright.config.ts
├── .eslintrc.cjs
├── .prettierrc
├── LICENSE                          # MIT
├── README.md
├── CONTRIBUTING.md
└── SECURITY.md
```

---

## 8. Development Milestones

### Phase 1 — Core File Manager (Weeks 1–6)

- [ ] Tauri v2 project scaffold with React + TypeScript + Vite
- [ ] Sidebar navigation (bookmarks, recents, drives)
- [ ] File view (grid, list, column, gallery) with virtualization
- [ ] Breadcrumb path bar
- [ ] Standard file operations (create, rename, move, copy, delete)
- [ ] Undo/redo stack
- [ ] Tabbed browsing
- [ ] Drag-and-drop
- [ ] Keyboard shortcuts (platform-adaptive)
- [ ] Light/dark theme with system detection
- [ ] Quick Look previews (images, PDF, code, markdown)

### Phase 2 — Local Intelligence (Weeks 7–10)

- [ ] SQLite database setup with migrations
- [ ] File metadata indexing with file watcher
- [ ] FTS5 keyword search
- [ ] OCR engine integration (Tesseract via leptess)
- [ ] Local embedding generation (fastembed-rs)
- [ ] sqlite-vec vector index
- [ ] Keyword-first search with semantic fallback
- [ ] Universal search bar with instant results

### Phase 3 — AI Integration (Weeks 11–14)

- [ ] Onboarding wizard (welcome, API key, permissions, privacy)
- [ ] Secure API key storage (OS keychain)
- [ ] Chat sidebar UI
- [ ] Claude API client with streaming
- [ ] Context assembly (directory, selected files, history)
- [ ] Suggest mode with diff preview and approval
- [ ] Auto-execute mode
- [ ] AI commands: move, rename, delete, summarize, search
- [ ] Permission filter and audit logging

### Phase 4 — Polish & Release (Weeks 15–18)

- [ ] Content-based categorization
- [ ] Duplicate detection (hash + similarity)
- [ ] Large/old file surfacing
- [ ] Project detection
- [ ] Smart folders
- [ ] Audio and video preview improvements
- [ ] Performance optimization and benchmarking
- [ ] E2E tests on all platforms
- [ ] CI/CD with GitHub Actions
- [ ] Cross-platform builds and code signing
- [ ] README, CONTRIBUTING, SECURITY docs
- [ ] v1.0 release
