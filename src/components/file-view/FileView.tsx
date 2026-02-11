import { useState, useCallback } from "react";
import { useSettingsStore } from "../../stores/settingsStore";
import { useFileStore } from "../../stores/fileStore";
import { useFileOperations } from "../../hooks/useFileOperations";
import { ListView } from "./ListView";
import { GridView } from "./GridView";
import { ContextMenu, type ContextMenuItem } from "../shared/ContextMenu";

export function FileView() {
  const viewMode = useSettingsStore((s) => s.viewMode);
  const sortedEntries = useFileStore((s) => s.sortedEntries);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const selectedFiles = useFileStore((s) => s.selectedFiles);
  const error = useFileStore((s) => s.error);
  const { createDir, deleteFiles, rename } = useFileOperations();

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);

  const entries = sortedEntries();

  function handleNavigate(entry: { is_directory: boolean; path: string }) {
    if (entry.is_directory) navigateTo(entry.path);
  }

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

  return (
    <div className="h-full overflow-auto" onContextMenu={handleContextMenu}>
      {error && <div className="p-4 text-red-500">{error}</div>}
      {viewMode === "grid" ? (
        <GridView entries={entries} onNavigate={handleNavigate} />
      ) : (
        <ListView entries={entries} onNavigate={handleNavigate} />
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
