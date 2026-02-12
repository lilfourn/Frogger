# Frogger 🐸 — System Design & Architecture

## Overview

This document describes the full system design for Frogger, covering the layered architecture, inter-process communication model, data flows, and key subsystem designs.

---

## 1. High-Level Architecture

Frogger follows Tauri v2's **multi-process architecture** with strict separation between the frontend WebView process and the Rust core process.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        FROGGER APPLICATION                         │
├───────────────────────────────┬─────────────────────────────────────┤
│     FRONTEND (WebView)        │        CORE PROCESS (Rust)          │
│                               │                                     │
│  ┌─────────────────────────┐  │  ┌───────────────────────────────┐  │
│  │     React + TypeScript  │  │  │     Tauri Command Handlers    │  │
│  │                         │  │  │                               │  │
│  │  ┌───────┐ ┌─────────┐ │  │  │  file_commands                │  │
│  │  │Sidebar│ │ MainView│ │◄─┼──┤  ai_commands                  │  │
│  │  └───────┘ └─────────┘ │  │  │  search_commands              │  │
│  │  ┌───────┐ ┌─────────┐ │  │  │  ocr_commands                 │  │
│  │  │ Chat  │ │QuickLook│ │  │  │  perm_commands                │  │
│  │  └───────┘ └─────────┘ │  │  └──────────┬────────────────────┘  │
│  │                         │  │             │                       │
│  └─────────────────────────┘  │  ┌──────────▼────────────────────┐  │
│                               │  │       Service Layer            │  │
│          Tauri IPC            │  │                               │  │
│     (Commands + Events)       │  │  FileService    AIService     │  │
│    ◄──────────────────────►   │  │  SearchService  OcrService    │  │
│                               │  │  UndoService    PermService   │  │
│                               │  │  EmbeddingService             │  │
│                               │  └──────────┬────────────────────┘  │
│                               │             │                       │
│                               │  ┌──────────▼────────────────────┐  │
│                               │  │       Data Layer              │  │
│                               │  │                               │  │
│                               │  │  SQLite + sqlite-vec          │  │
│                               │  │  OS Keychain                  │  │
│                               │  │  Shell Executor               │  │
│                               │  │  File Watcher (notify)        │  │
│                               │  └───────────────────────────────┘  │
├───────────────────────────────┴─────────────────────────────────────┤
│                         EXTERNAL SERVICES                           │
│                    Anthropic Claude API (HTTPS)                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

| Layer | Responsibility |
|---|---|
| **Frontend (WebView)** | UI rendering, user interaction, state management, animations |
| **Tauri IPC** | Type-safe command invocation and event broadcasting between processes |
| **Command Handlers** | Thin routing layer — validates input, delegates to services |
| **Service Layer** | All business logic — file ops, AI orchestration, search, OCR |
| **Data Layer** | Persistence (SQLite), secrets (keychain), OS integration (shell, fs watcher) |
| **External** | Anthropic Claude API over HTTPS |

---

## 2. Tauri IPC Design

Tauri v2 uses a **command-based IPC** model. The frontend calls Rust functions via `invoke()`, and Rust emits events back to the frontend via `emit()`.

### Command Pattern

```rust
// src-tauri/src/commands/file_commands.rs
#[tauri::command]
async fn list_directory(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FileEntry>, AppError> {
    let perm_service = &state.permission_service;
    perm_service.check_read_access(&path)?;

    let file_service = &state.file_service;
    file_service.list_directory(&path).await
}

#[tauri::command]
async fn move_files(
    sources: Vec<String>,
    destination: String,
    state: tauri::State<'_, AppState>,
) -> Result<OperationResult, AppError> {
    let perm_service = &state.permission_service;
    perm_service.check_write_access(&destination)?;

    let file_service = &state.file_service;
    let result = file_service.move_files(&sources, &destination).await?;

    // Log to undo stack
    state.undo_service.push(result.operation_record.clone());

    Ok(result)
}
```

### Frontend Invocation

```typescript
// src/services/fileService.ts
import { invoke } from '@tauri-apps/api/core';

export async function listDirectory(path: string): Promise<FileEntry[]> {
  return invoke<FileEntry[]>('list_directory', { path });
}

export async function moveFiles(
  sources: string[],
  destination: string
): Promise<OperationResult> {
  return invoke<OperationResult>('move_files', { sources, destination });
}
```

### Event System

Events flow from Rust → Frontend for real-time updates:

```rust
// Rust: Emit progress during long operations
app_handle.emit("indexing-progress", IndexProgress {
    files_processed: 150,
    total_files: 500,
    current_file: "document.pdf".into(),
})?;

// Rust: Emit file system changes
app_handle.emit("fs-change", FsChangeEvent {
    event_type: FsEventType::Modified,
    path: "/Users/luke/Documents/report.pdf".into(),
})?;
```

```typescript
// Frontend: Listen for events
import { listen } from '@tauri-apps/api/event';

listen<IndexProgress>('indexing-progress', (event) => {
  setProgress(event.payload);
});

listen<FsChangeEvent>('fs-change', (event) => {
  refreshDirectory(event.payload.path);
});
```

---

## 3. Frontend Component Architecture

```
App
├── AppLayout
│   ├── TitleBar (custom, platform-adaptive)
│   ├── Sidebar
│   │   ├── NavTree (bookmarks, recents, drives)
│   │   ├── Favorites
│   │   ├── ConnectedDrives
│   │   └── SmartFolders (AI-generated categories)
│   ├── MainPanel
│   │   ├── Toolbar (view toggles, sort, new folder, etc.)
│   │   ├── Breadcrumb (path bar with editable input)
│   │   ├── FileView (polymorphic)
│   │   │   ├── GridView (icon grid)
│   │   │   ├── ListView (detailed list)
│   │   │   ├── ColumnView (Miller columns)
│   │   │   └── GalleryView (large previews)
│   │   └── StatusBar (item count, disk space)
│   ├── QuickLookPanel (overlay/split)
│   │   ├── ImagePreview
│   │   ├── VideoPreview
│   │   ├── PdfPreview
│   │   ├── CodePreview
│   │   ├── MarkdownPreview
│   │   └── AudioPreview
│   └── ChatSidebar (collapsible, right side)
│       ├── ChatHeader (mode toggle, settings)
│       ├── MessageList (virtualized)
│       ├── DiffPreview (proposed changes)
│       ├── ApprovalBar (approve/reject/edit)
│       └── ChatInput (with file context indicator)
├── OnboardingWizard
│   ├── WelcomeStep
│   ├── ApiKeyStep
│   ├── PermissionStep
│   └── PrivacySummaryStep
└── SettingsModal
    ├── GeneralSettings
    ├── AppearanceSettings
    ├── PermissionSettings
    ├── PrivacyLogViewer
    └── ApiKeyManager
```

### State Management (Zustand)

```typescript
// src/stores/fileStore.ts
interface FileStore {
  currentPath: string;
  entries: FileEntry[];
  selectedFiles: Set<string>;
  viewMode: 'grid' | 'list' | 'column' | 'gallery';
  sortBy: SortField;
  sortDirection: 'asc' | 'desc';

  // Actions
  navigateTo: (path: string) => Promise<void>;
  selectFile: (path: string, multi?: boolean) => void;
  setViewMode: (mode: ViewMode) => void;
}

// src/stores/chatStore.ts
interface ChatStore {
  messages: ChatMessage[];
  isStreaming: boolean;
  approvalMode: 'suggest' | 'auto';
  pendingOperations: ProposedOperation[] | null;

  // Actions
  sendMessage: (content: string) => Promise<void>;
  approveOperations: () => Promise<void>;
  rejectOperations: () => void;
}

// src/stores/undoStore.ts
interface UndoStore {
  canUndo: boolean;
  canRedo: boolean;
  lastOperation: string | null;

  undo: () => Promise<void>;
  redo: () => Promise<void>;
}
```

---

## 4. AI Chat Architecture

### Context Assembly

When the user sends a message, the frontend assembles a context object:

```typescript
interface ChatContext {
  currentDirectory: string;
  selectedFiles: FileEntry[];      // Currently selected files
  visibleFiles: FileEntry[];       // Files in current view
  navigationHistory: string[];     // Recent directories visited
  previousMessages: ChatMessage[]; // Conversation history
}
```

### Claude System Prompt Design

```
You are Frogger's AI assistant embedded in a cross-platform file manager.
You help users organize, search, rename, and manage their files.

CURRENT CONTEXT:
- Directory: {currentDirectory}
- Selected files: {selectedFiles}
- Platform: {platform} (macOS|Windows|Linux)

CAPABILITIES:
- Generate shell commands for file operations
- Analyze file contents when permission is granted
- Search the semantic index
- Categorize and tag files

RULES:
- Always generate platform-appropriate commands ({bash|powershell})
- Never execute destructive operations without explicit approval
- When in "suggest" mode, return a structured JSON operation plan
- Respect permission scopes — never access directories outside allowed paths

OUTPUT FORMAT (for file operations):
Return a JSON object with:
{
  "explanation": "Human-readable description of what will happen",
  "operations": [
    {
      "type": "move|copy|rename|delete|mkdir",
      "source": "/path/to/source",
      "destination": "/path/to/dest",
      "command": "mv '/path/source' '/path/dest'",
      "inverse_command": "mv '/path/dest' '/path/source'"
    }
  ]
}
```

### AI Request Pipeline

```
User Input
    │
    ▼
┌─────────────────────┐
│  Context Assembler   │  Gathers selected files, current dir, permissions
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Permission Filter   │  Strips file paths/content outside allowed scopes
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Prompt Builder      │  Constructs system prompt + user message + context
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Claude API Client   │  Streams response via SSE (reqwest)
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Response Parser     │  Extracts operation plan JSON or conversational text
└──────────┬──────────┘
           │
    ┌──────┴──────┐
    ▼             ▼
 Suggest       Auto-Execute
 Mode          Mode
    │             │
    ▼             ▼
 Show Diff     Execute → Log → Notify
 → Await
 Approval
    │
    ▼
 Execute → Log → Notify
```

---

## 5. Undo/Redo System

The undo system uses the **Command Pattern** where every file operation records both the forward command and its inverse.

### Operation Record

```rust
pub struct OperationRecord {
    pub id: Uuid,
    pub operation_type: OperationType,
    pub forward_command: ShellCommand,    // The command that was executed
    pub inverse_command: ShellCommand,    // The command to reverse it
    pub affected_paths: Vec<PathBuf>,
    pub metadata: serde_json::Value,      // Additional context
    pub executed_at: chrono::DateTime<Utc>,
}

pub enum OperationType {
    Move,
    Copy,
    Rename,
    Delete,     // inverse = restore from trash/backup
    CreateDir,
    BatchRename,
    AiOperation { session_id: String },
}
```

### Undo Stack Architecture

```
                    ┌─────────────┐
 User Action ──►   │  Execute     │
                    │  Command     │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐     ┌─────────────┐
                    │  Undo Stack │     │  Redo Stack  │
                    │  (LIFO)     │     │  (LIFO)      │
                    │             │     │              │
                    │  Op N  ◄────┼─────┼── (cleared   │
                    │  Op N-1     │     │   on new     │
                    │  Op N-2     │     │   action)    │
                    │  ...        │     │              │
                    └─────────────┘     └──────────────┘

 Undo ──► Pop from Undo Stack, execute inverse_command, push to Redo Stack
 Redo ──► Pop from Redo Stack, execute forward_command, push to Undo Stack
```

### Delete Safety

Deletes are **never immediate `rm`**. Instead:

1. **Soft delete**: Move file to `~/.frogger/trash/{uuid}/` with metadata JSON
2. **Undo**: Move file back from Frogger trash to original location
3. **Hard delete**: Only on explicit "Empty Trash" or after configurable retention (default 30 days)

---

## 6. Shell Execution Layer

```rust
pub struct ShellExecutor {
    platform: Platform,
}

impl ShellExecutor {
    pub async fn execute(&self, command: &ShellCommand) -> Result<CommandOutput> {
        match self.platform {
            Platform::MacOS | Platform::Linux => {
                self.execute_bash(&command.unix_form).await
            }
            Platform::Windows => {
                self.execute_powershell(&command.windows_form).await
            }
        }
    }
}

pub struct ShellCommand {
    pub unix_form: String,       // bash command
    pub windows_form: String,    // PowerShell equivalent
    pub requires_confirmation: bool,
    pub is_destructive: bool,
}
```

### Safety Rails

| Rail | Description |
|---|---|
| **Destructive confirmation** | Any `rm`, `del`, or overwrite prompts for confirmation |
| **Path validation** | Commands are validated to ensure they only touch allowed directories |
| **No root operations** | Commands targeting `/`, `C:\`, or system directories are blocked |
| **Command sanitization** | Input is escaped to prevent shell injection |
| **Timeout** | Commands have a configurable timeout (default 30s) |
| **Dry-run mode** | `--dry-run` or `-WhatIf` flags for preview when available |

---

## 7. Indexing & Semantic Search

### Ingestion Pipeline

```
File System Watcher (notify crate)
        │
        ▼
┌───────────────────┐
│  Change Detected   │  (create / modify / delete)
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  Metadata Extract  │  name, size, dates, MIME type, EXIF
└───────┬───────────┘
        │
   ┌────┴────┐
   ▼         ▼
 Image/    Other
 PDF?      Files
   │         │
   ▼         │
┌─────────┐  │
│  OCR    │  │
│(leptess)│  │
└────┬────┘  │
     │       │
     └───┬───┘
         ▼
┌───────────────────┐
│  Text Assembly     │  Combine: filename + metadata + OCR text + tags
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  Embedding Gen     │  fastembed-rs (all-MiniLM-L6-v2, 384 dims)
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  SQLite Write      │  files table + vec_index + FTS5
└───────────────────┘
```

### Hybrid Search (Keyword + Semantic)

```rust
pub async fn search(query: &str, options: SearchOptions) -> Vec<SearchResult> {
    // 1. FTS5 keyword search
    let keyword_results = db.query(
        "SELECT id, path, rank FROM files_fts WHERE files_fts MATCH ?",
        [query]
    );

    // 2. Generate query embedding
    let query_embedding = embedding_service.embed(query).await?;

    // 3. Vector similarity search
    let vector_results = db.query(
        "SELECT file_id, distance FROM vec_index
         WHERE embedding MATCH ? ORDER BY distance LIMIT 50",
        [query_embedding]
    );

    // 4. Reciprocal Rank Fusion (RRF)
    let fused = reciprocal_rank_fusion(keyword_results, vector_results, k=60);

    // 5. Optional: Claude re-ranking for ambiguous queries
    if options.deep_search {
        let reranked = ai_service.rerank(query, &fused).await?;
        return reranked;
    }

    fused
}
```

---

## 8. OCR Subsystem

### Processing Pipeline

```
Directory Navigated
        │
        ▼
┌─────────────────────────┐
│  Scan visible files for  │
│  images & PDFs           │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  Check cache: was file   │  Compare file modified_at vs processed_at
│  already processed?      │
└───────────┬─────────────┘
            │ (cache miss)
            ▼
┌─────────────────────────┐
│  Queue for background    │  Tokio task pool (bounded concurrency = 4)
│  OCR processing          │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  leptess OCR engine      │  Tesseract with eng + user-configured languages
│  Extract text            │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  Store in ocr_text table │  Index in FTS5 + generate embedding
│  Update file thumbnail   │  Add "OCR available" indicator
└─────────────────────────┘
```

### Key Design Decisions

- **Lazy processing**: OCR only runs on files in the current directory view (not full-disk scans)
- **Background threads**: Uses a bounded Tokio task pool to avoid blocking the UI
- **Cache invalidation**: Re-processes only when `file.modified_at > ocr.processed_at`
- **Configurable**: Can be disabled globally or per-directory via permission scopes

---

## 9. Security Architecture

### Threat Model

| Threat | Mitigation |
|---|---|
| API key theft | Stored in OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service) |
| Shell injection | All shell commands use parameterized execution, not string concatenation |
| Unauthorized file access | Tauri's scoped FS permissions + Frogger's permission_scopes table |
| Data exfiltration via Claude | Permission filter strips disallowed paths before API calls; full audit log |
| Malicious AI output | All AI-generated commands require approval in suggest mode; destructive ops always confirmed |

### Tauri Capabilities Configuration

```json
// src-tauri/capabilities/default.json
{
  "identifier": "default",
  "description": "Default capability set for Frogger",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-execute",
    "shell:allow-open",
    "fs:allow-read",
    "fs:allow-write",
    "dialog:allow-open",
    "dialog:allow-save",
    "notification:default",
    "os:default",
    {
      "identifier": "fs:scope",
      "allow": [
        { "path": "$HOME/**" },
        { "path": "$DOCUMENT/**" },
        { "path": "$DOWNLOAD/**" },
        { "path": "$DESKTOP/**" }
      ],
      "deny": [
        { "path": "$HOME/.ssh/**" },
        { "path": "$HOME/.gnupg/**" }
      ]
    }
  ]
}
```

---

## 10. Cross-Platform Abstractions

| Concern | macOS | Windows | Linux |
|---|---|---|---|
| Shell | `/bin/zsh` or `/bin/bash` | `powershell.exe` | `/bin/bash` |
| Keychain | macOS Keychain | Windows Credential Manager | `secret-service` (D-Bus) |
| Trash | `~/.Trash` | Recycle Bin (via shell) | `freedesktop.org` trash spec |
| File Watcher | FSEvents via `notify` | ReadDirectoryChangesW | inotify |
| Native theme | `NSAppearance` | Windows accent color | GTK/system theme |
| Shortcuts | `Cmd+` prefix | `Ctrl+` prefix | `Ctrl+` prefix |

---

## 11. Performance Targets

| Metric | Target |
|---|---|
| App cold start | < 1.5 seconds |
| Directory listing (1,000 files) | < 200ms |
| Directory listing (10,000 files) | < 800ms (virtualized rendering) |
| Search query (local index) | < 100ms |
| Embedding generation (single file) | < 50ms |
| OCR (single page image) | < 2 seconds |
| Memory usage (idle) | < 120MB |
| Bundle size | < 15MB |
