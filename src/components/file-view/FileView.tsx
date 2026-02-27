import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { useSettingsStore } from "../../stores/settingsStore";
import { useFileStore } from "../../stores/fileStore";
import { useFileOperations } from "../../hooks/useFileOperations";
import { useFileNavigation } from "../../hooks/useFileNavigation";
import { openFile } from "../../services/fileService";
import { ListView } from "./ListView";
import { GridView } from "./GridView";
import { ColumnView } from "./ColumnView";
import { GalleryView } from "./GalleryView";
import { ContextMenu, type ContextMenuItem } from "../shared/ContextMenu";
import { useChat } from "../../hooks/useChat";
import { ShieldAlert, AlertCircle, RotateCcw } from "lucide-react";
import type { FileEntry } from "../../types/file";

export function FileView() {
  const viewMode = useSettingsStore((s) => s.viewMode);
  const sortedEntries = useFileStore((s) => s.sortedEntries);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const addTab = useFileStore((s) => s.addTab);
  const selectedFiles = useFileStore((s) => s.selectedFiles);
  const setSelectedFiles = useFileStore((s) => s.setSelectedFiles);
  const error = useFileStore((s) => s.error);
  const clearError = useFileStore((s) => s.clearError);
  const { refresh, createDir, deleteFiles, rename } = useFileOperations();
  const { startOrganize } = useChat();

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);
  const [contextTarget, setContextTarget] = useState<FileEntry | null>(null);

  const entries = sortedEntries();
  const { focusIndex, moveDown, moveUp, focusedEntry } = useFileNavigation(entries);
  const containerRef = useRef<HTMLDivElement>(null);
  const isPermissionError = error?.toLowerCase().includes("permission");

  const handleRetry = useCallback(() => {
    clearError();
    refresh();
  }, [clearError, refresh]);

  const selectedPaths = useMemo(() => new Set(selectedFiles), [selectedFiles]);

  useEffect(() => {
    if (focusedEntry) {
      setSelectedFiles([focusedEntry.path]);
    }
  }, [focusedEntry, setSelectedFiles]);

  const handleSelect = useCallback(
    (entry: FileEntry) => {
      setSelectedFiles([entry.path]);
    },
    [setSelectedFiles],
  );

  const handleOpen = useCallback(
    (entry: FileEntry) => {
      if (entry.is_directory) {
        navigateTo(entry.path);
      } else {
        openFile(entry.path).catch((err) => console.error("[FileView] Failed to open file:", err));
      }
    },
    [navigateTo],
  );

  const handleItemContextMenu = useCallback(
    (e: React.MouseEvent, entry: FileEntry) => {
      e.preventDefault();
      e.stopPropagation();
      setSelectedFiles([entry.path]);
      setContextTarget(entry);
      setContextMenu({ x: e.clientX, y: e.clientY });
    },
    [setSelectedFiles],
  );

  const handleBackgroundContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setContextTarget(null);
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, []);

  const contextItems: ContextMenuItem[] = contextTarget
    ? [
        {
          label: "Open",
          action: () => handleOpen(contextTarget),
        },
        ...(contextTarget.is_directory
          ? [
              {
                label: "Open in New Tab",
                action: () => {
                  addTab();
                  navigateTo(contextTarget.path);
                },
              },
              {
                label: "Organize with AI",
                action: () => startOrganize(contextTarget.path),
              },
            ]
          : []),
        { separator: true },
        {
          label: "Rename",
          shortcut: "F2",
          action: () => {
            const newName = prompt("New name:");
            if (newName) rename(contextTarget.path, newName);
          },
        },
        {
          label: "Copy Path",
          action: () => {
            navigator.clipboard
              .writeText(contextTarget.path)
              .catch((err) => console.error("[FileView] Clipboard write failed:", err));
          },
        },
        { separator: true },
        {
          label: "Delete",
          shortcut: "\u2318\u232B",
          destructive: true,
          action: () => deleteFiles([contextTarget.path]),
        },
      ]
    : [
        {
          label: "New Folder",
          shortcut: "\u21E7\u2318N",
          action: () => {
            const name = prompt("Folder name:");
            if (name) createDir(name);
          },
        },
      ];

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          moveDown();
          break;
        case "ArrowUp":
          e.preventDefault();
          moveUp();
          break;
        case "Enter":
          if (focusedEntry) handleOpen(focusedEntry);
          break;
      }
    },
    [moveDown, moveUp, focusedEntry, handleOpen],
  );

  const viewProps = {
    entries,
    onSelect: handleSelect,
    onOpen: handleOpen,
    onItemContextMenu: handleItemContextMenu,
    selectedPaths,
    focusIndex,
  };

  return (
    <div
      ref={containerRef}
      className="h-full overflow-auto outline-none"
      onContextMenu={handleBackgroundContextMenu}
      onKeyDown={handleKeyDown}
      tabIndex={0}
      role="list"
      aria-label="File list"
    >
      {error && entries.length === 0 ? (
        <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
          {isPermissionError ? (
            <ShieldAlert size={40} className="text-[var(--color-text-secondary)]" />
          ) : (
            <AlertCircle size={40} className="text-[var(--color-text-secondary)]" />
          )}
          <p className="text-sm text-[var(--color-text-secondary)]">
            {isPermissionError
              ? "Permission required to view this directory"
              : "Failed to load directory contents"}
          </p>
          <button
            type="button"
            onClick={handleRetry}
            className="mt-2 flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-xs text-white hover:opacity-90"
          >
            <RotateCcw size={12} />
            Retry
          </button>
        </div>
      ) : (
        <>
          {error && (
            <div className="flex items-center gap-2 border-b border-[var(--color-border)] bg-red-500/10 px-4 py-2 text-xs text-red-500">
              <AlertCircle size={14} />
              {error}
            </div>
          )}
          {viewMode === "grid" && <GridView {...viewProps} />}
          {viewMode === "list" && <ListView {...viewProps} />}
          {viewMode === "column" && <ColumnView {...viewProps} />}
          {viewMode === "gallery" && <GalleryView {...viewProps} />}
        </>
      )}
      {contextMenu && (
        <ContextMenu
          items={contextItems}
          position={contextMenu}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}
