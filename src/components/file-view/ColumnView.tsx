import { useState, useCallback, useEffect, useRef } from "react";
import type { FileEntry } from "../../types/file";
import { FileIcon } from "../shared/FileIcon";
import { listDirectory } from "../../services/fileService";
import { useFileStore } from "../../stores/fileStore";

interface Column {
  path: string;
  entries: FileEntry[];
  selectedPath: string | null;
}

interface ColumnViewProps {
  entries: FileEntry[];
  onSelect: (entry: FileEntry) => void;
  onOpen: (entry: FileEntry) => void;
  onItemContextMenu: (e: React.MouseEvent, entry: FileEntry) => void;
  selectedPaths: Set<string>;
  focusIndex?: number;
}

export function ColumnView({
  entries,
  onSelect,
  onOpen,
  onItemContextMenu,
  selectedPaths,
}: ColumnViewProps) {
  const currentPath = useFileStore((s) => s.currentPath);
  const navigateTo = useFileStore((s) => s.navigateTo);

  const [columns, setColumns] = useState<Column[]>([
    { path: currentPath, entries, selectedPath: null },
  ]);
  const [activeColumn, setActiveColumn] = useState(0);
  const [prevEntries, setPrevEntries] = useState(entries);
  const [selfNavPath, setSelfNavPath] = useState<string | null>(null);
  const lastColumnRef = useRef<HTMLDivElement>(null);

  if (prevEntries !== entries) {
    setPrevEntries(entries);
    if (selfNavPath === currentPath) {
      if (entries.length > 0) {
        setSelfNavPath(null);
      }
    } else {
      setColumns([{ path: currentPath, entries, selectedPath: null }]);
      setActiveColumn(0);
    }
  }

  useEffect(() => {
    lastColumnRef.current?.scrollIntoView?.({ behavior: "smooth", inline: "end" });
  }, [columns.length]);

  const handleEntryClick = useCallback(
    async (columnIndex: number, entry: FileEntry) => {
      if (entry.is_directory) {
        try {
          const children = await listDirectory(entry.path);
          setColumns((prev) => {
            const updated = prev.slice(0, columnIndex + 1);
            updated[columnIndex] = { ...updated[columnIndex], selectedPath: entry.path };
            updated.push({ path: entry.path, entries: children, selectedPath: null });
            return updated;
          });
          setActiveColumn(columnIndex + 1);
          setSelfNavPath(entry.path);
          navigateTo(entry.path);
        } catch {
          // listDirectory handles permission prompts
        }
      } else {
        setColumns((prev) => {
          const updated = prev.slice(0, columnIndex + 1);
          updated[columnIndex] = { ...updated[columnIndex], selectedPath: entry.path };
          return updated;
        });
        setActiveColumn(columnIndex);
        onSelect(entry);
      }
    },
    [navigateTo, onSelect],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const col = columns[activeColumn];
      if (!col) return;

      const currentIdx = col.entries.findIndex((en) => en.path === col.selectedPath);

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          e.stopPropagation();
          const next = Math.min(currentIdx + 1, col.entries.length - 1);
          const entry = col.entries[next];
          if (entry) {
            setColumns((prev) => {
              const updated = [...prev];
              updated[activeColumn] = { ...updated[activeColumn], selectedPath: entry.path };
              return updated;
            });
            onSelect(entry);
          }
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          e.stopPropagation();
          const prev = Math.max(currentIdx - 1, 0);
          const entry = col.entries[prev];
          if (entry) {
            setColumns((p) => {
              const updated = [...p];
              updated[activeColumn] = { ...updated[activeColumn], selectedPath: entry.path };
              return updated;
            });
            onSelect(entry);
          }
          break;
        }
        case "ArrowRight": {
          e.preventDefault();
          e.stopPropagation();
          const selected = col.entries.find((en) => en.path === col.selectedPath);
          if (selected?.is_directory) {
            handleEntryClick(activeColumn, selected);
          } else if (activeColumn < columns.length - 1) {
            setActiveColumn(activeColumn + 1);
          }
          break;
        }
        case "ArrowLeft": {
          e.preventDefault();
          e.stopPropagation();
          if (activeColumn > 0) {
            setActiveColumn(activeColumn - 1);
          }
          break;
        }
        case "Enter": {
          e.stopPropagation();
          const selected = col.entries.find((en) => en.path === col.selectedPath);
          if (selected) onOpen(selected);
          break;
        }
      }
    },
    [columns, activeColumn, onSelect, onOpen, handleEntryClick],
  );

  return (
    <div
      data-testid="column-view"
      className="flex h-full overflow-x-auto outline-none"
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      {columns.map((col, colIdx) => (
        <div
          key={col.path}
          ref={colIdx === columns.length - 1 ? lastColumnRef : undefined}
          className="min-w-[220px] max-w-[280px] shrink-0 overflow-y-auto border-r border-[var(--color-border)]"
        >
          {col.entries.map((entry) => (
            <div
              key={entry.path}
              role="listitem"
              className={`flex cursor-pointer items-center justify-between px-3 py-1.5 hover:bg-[var(--color-bg-secondary)] ${
                selectedPaths.has(entry.path) || col.selectedPath === entry.path
                  ? "bg-[var(--color-accent)]/10"
                  : ""
              } ${colIdx === activeColumn && col.selectedPath === entry.path ? "bg-[var(--color-accent)]/20" : ""}`}
              onClick={() => handleEntryClick(colIdx, entry)}
              onDoubleClick={() => onOpen(entry)}
              onContextMenu={(e) => onItemContextMenu(e, entry)}
            >
              <div className="flex items-center gap-2 overflow-hidden">
                <FileIcon isDirectory={entry.is_directory} size={16} />
                <span className="truncate text-sm">{entry.name}</span>
              </div>
              {entry.is_directory && (
                <span className="ml-2 text-xs text-[var(--color-text-secondary)]">&gt;</span>
              )}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
