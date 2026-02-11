import { useState, useCallback, useEffect, useRef } from "react";
import { useSettingsStore } from "../../stores/settingsStore";
import { useFileStore } from "../../stores/fileStore";
import { useFileOperations } from "../../hooks/useFileOperations";
import { useFileNavigation } from "../../hooks/useFileNavigation";
import { ListView } from "./ListView";
import { GridView } from "./GridView";
import { ContextMenu, type ContextMenuItem } from "../shared/ContextMenu";

export function FileView() {
  const viewMode = useSettingsStore((s) => s.viewMode);
  const sortedEntries = useFileStore((s) => s.sortedEntries);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const selectedFiles = useFileStore((s) => s.selectedFiles);
  const setSelectedFiles = useFileStore((s) => s.setSelectedFiles);
  const error = useFileStore((s) => s.error);
  const { createDir, deleteFiles, rename } = useFileOperations();

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);

  const entries = sortedEntries();
  const { focusIndex, moveDown, moveUp, focusedEntry } = useFileNavigation(entries);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (focusedEntry) {
      setSelectedFiles([focusedEntry.path]);
    }
  }, [focusedEntry, setSelectedFiles]);

  const handleNavigate = useCallback(
    (entry: { is_directory: boolean; path: string }) => {
      if (entry.is_directory) navigateTo(entry.path);
    },
    [navigateTo],
  );

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, []);

  const contextItems: ContextMenuItem[] = [
    {
      label: "New Folder",
      shortcut: "\u21E7\u2318N",
      action: () => {
        const name = prompt("Folder name:");
        if (name) createDir(name);
      },
    },
    { separator: true },
    ...(selectedFiles.length > 0
      ? [
          {
            label: "Rename",
            shortcut: "F2",
            action: () => {
              const newName = prompt("New name:");
              if (newName) rename(selectedFiles[0], newName);
            },
          },
          { separator: true } as ContextMenuItem,
          {
            label: "Delete",
            shortcut: "\u2318\u232B",
            destructive: true,
            action: () => deleteFiles(selectedFiles),
          },
        ]
      : []),
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
          if (focusedEntry) handleNavigate(focusedEntry);
          break;
      }
    },
    [moveDown, moveUp, focusedEntry, handleNavigate],
  );

  return (
    <div
      ref={containerRef}
      className="h-full overflow-auto outline-none"
      onContextMenu={handleContextMenu}
      onKeyDown={handleKeyDown}
      tabIndex={0}
      role="list"
      aria-label="File list"
    >
      {error && <div className="p-4 text-red-500">{error}</div>}
      {viewMode === "grid" ? (
        <GridView entries={entries} onNavigate={handleNavigate} focusIndex={focusIndex} />
      ) : (
        <ListView entries={entries} onNavigate={handleNavigate} focusIndex={focusIndex} />
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
