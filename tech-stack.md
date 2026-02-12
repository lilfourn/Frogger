# Frogger 🐸 — Tech Stack Specification

## Overview

Frogger is an AI-native, cross-platform desktop file manager built on **Tauri v2 + React + TypeScript** with Claude AI integration. This document defines every technology choice, version, and rationale.

---

## Frontend Stack

| Technology | Version | Purpose |
|---|---|---|
| React | 19.x | Component-based UI framework |
| TypeScript | 5.x | Type-safe frontend development |
| Vite | 6.x | Build tool and dev server (Tauri default) |
| Tailwind CSS | 4.x | Utility-first styling for Apple-inspired design |
| Framer Motion | 11.x | Fluid animations and transitions |
| Zustand | 5.x | Lightweight global state management |
| TanStack Virtual | 3.x | Virtualized lists for large directories |
| React Aria | 3.x | Accessible UI primitives (keyboard nav, focus management) |
| CodeMirror | 6.x | Syntax-highlighted code previews |
| react-markdown | 9.x | Rendered markdown previews |
| PDF.js | 4.x | PDF rendering in Quick Look panel |
| WaveSurfer.js | 7.x | Audio waveform visualization |

### Frontend Dev Dependencies

| Technology | Version | Purpose |
|---|---|---|
| ESLint | 9.x | Linting with flat config |
| Prettier | 3.x | Code formatting |
| Vitest | 2.x | Unit testing for React components |
| Playwright | 1.x | E2E testing across platforms |
| Storybook | 8.x | Component development and documentation |

---

## Backend Stack (Rust / Tauri Core)

| Crate | Version | Purpose |
|---|---|---|
| `tauri` | 2.x | Application framework, IPC, window management |
| `tauri-plugin-shell` | 2.x | Shell command execution for file operations |
| `tauri-plugin-dialog` | 2.x | Native file dialogs |
| `tauri-plugin-fs` | 2.x | File system access with scoped permissions |
| `tauri-plugin-os` | 2.x | OS detection for platform-adaptive behavior |
| `tauri-plugin-notification` | 2.x | Native OS notifications |
| `rusqlite` | 0.32.x | SQLite database interface |
| `sqlite-vec` | 0.1.x | Vector similarity search extension for SQLite |
| `fastembed` | 4.x | Local ONNX-based embedding generation |
| `leptess` | 0.4.x | Tesseract OCR Rust bindings |
| `keyring` | 3.x | Cross-platform OS keychain access |
| `reqwest` | 0.12.x | HTTP client for Claude API calls |
| `tokio` | 1.x | Async runtime |
| `serde` / `serde_json` | 1.x | Serialization/deserialization |
| `notify` | 7.x | Cross-platform file system watcher |
| `sha2` | 0.10.x | File hashing for duplicate detection |
| `walkdir` | 2.x | Recursive directory traversal |
| `chrono` | 0.4.x | Timestamp handling |
| `tracing` | 0.1.x | Structured logging |
| `thiserror` | 2.x | Ergonomic error types |
| `uuid` | 1.x | Unique operation IDs for undo stack |
| `kamadak-exif` | 0.5.x | EXIF metadata extraction from images |
| `mime_guess` | 2.x | MIME type detection |

---

## Database Schema (SQLite)

### Core Tables

```sql
-- Files metadata cache
CREATE TABLE files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    extension TEXT,
    mime_type TEXT,
    size_bytes INTEGER,
    created_at TEXT,
    modified_at TEXT,
    accessed_at TEXT,
    hash_sha256 TEXT,
    is_directory BOOLEAN DEFAULT 0,
    parent_path TEXT,
    indexed_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_files_path ON files(path);
CREATE INDEX idx_files_parent ON files(parent_path);
CREATE INDEX idx_files_extension ON files(extension);
CREATE INDEX idx_files_hash ON files(hash_sha256);

-- OCR extracted text
CREATE TABLE ocr_text (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
    extracted_text TEXT NOT NULL,
    language TEXT DEFAULT 'eng',
    confidence REAL,
    processed_at TEXT DEFAULT CURRENT_TIMESTAMP,
    file_modified_at TEXT -- re-process if file changes
);

CREATE INDEX idx_ocr_file ON ocr_text(file_id);

-- AI-generated metadata
CREATE TABLE ai_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
    summary TEXT,
    category TEXT,
    tags TEXT, -- JSON array
    generated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Full-text search (FTS5)
CREATE VIRTUAL TABLE files_fts USING fts5(
    name,
    path,
    ocr_text,
    summary,
    tags,
    content='files',
    content_rowid='id'
);

-- Vector embeddings (sqlite-vec)
CREATE VIRTUAL TABLE vec_index USING vec0(
    file_id INTEGER,
    embedding FLOAT[384] -- all-MiniLM-L6-v2 dimension
);

-- Chat history
CREATE TABLE chat_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    context_directory TEXT,
    context_files TEXT, -- JSON array of selected file paths
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Undo log
CREATE TABLE undo_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT UNIQUE NOT NULL,
    operation_type TEXT NOT NULL,
    forward_command TEXT NOT NULL,
    inverse_command TEXT NOT NULL,
    affected_paths TEXT NOT NULL, -- JSON array
    metadata TEXT, -- JSON object
    executed_at TEXT DEFAULT CURRENT_TIMESTAMP,
    undone BOOLEAN DEFAULT 0
);

CREATE INDEX idx_undo_time ON undo_log(executed_at DESC);

-- Permission scopes
CREATE TABLE permission_scopes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    directory_path TEXT UNIQUE NOT NULL,
    allow_content_scan BOOLEAN DEFAULT 0,
    allow_modification BOOLEAN DEFAULT 0,
    allow_ocr BOOLEAN DEFAULT 1,
    allow_indexing BOOLEAN DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- API call audit log
CREATE TABLE api_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT DEFAULT CURRENT_TIMESTAMP,
    endpoint TEXT NOT NULL,
    model TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    file_paths_sent TEXT, -- JSON array
    request_summary TEXT
);
```

---

## Embedding Model

| Property | Value |
|---|---|
| Model | `sentence-transformers/all-MiniLM-L6-v2` |
| Runtime | ONNX via `fastembed-rs` |
| Dimensions | 384 |
| Runs on | CPU (local, no GPU required) |
| Index | `sqlite-vec` with cosine similarity |

This model was chosen for its small size (~23MB), fast inference on CPU, and strong semantic quality for file search use cases.

---

## Claude API Integration

| Property | Value |
|---|---|
| Provider | Anthropic |
| Model (default) | `claude-sonnet-4-20250514` |
| Model (fallback) | `claude-haiku-3-20250305` |
| Auth | BYOK — user-supplied API key |
| Key Storage | OS keychain via `keyring` crate |
| Transport | HTTPS via `reqwest` with streaming SSE |
| Rate Limiting | Client-side token bucket (configurable) |

---

## Build & Distribution

| Tool | Purpose |
|---|---|
| `cargo` | Rust dependency management and build |
| `pnpm` | Frontend package manager (faster, disk-efficient) |
| `tauri-cli` | Build orchestration, bundling, signing |
| GitHub Actions | CI/CD for all three platforms |
| `tauri-action` | GitHub Action for cross-platform builds |

### Bundle Sizes (Target)

| Platform | Format | Target Size |
|---|---|---|
| macOS | `.dmg` / `.app` | ~8-12 MB |
| Windows | `.msi` / `.exe` (NSIS) | ~8-12 MB |
| Linux | `.deb` / `.AppImage` | ~8-12 MB |

---

## Supported Platforms

| Platform | Minimum Version | Shell |
|---|---|---|
| macOS | 11.0 (Big Sur) | Bash / Zsh |
| Windows | 10 (1803+) | PowerShell 5.1+ |
| Linux | Ubuntu 22.04 / Fedora 38+ | Bash |

---

## License

| Component | License |
|---|---|
| Frogger Application | MIT |
| Tauri | MIT / Apache 2.0 |
| Tesseract OCR | Apache 2.0 |
| `all-MiniLM-L6-v2` | Apache 2.0 |
| SQLite | Public Domain |
