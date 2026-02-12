import type { FileEntry } from "../../types/file";
import { FileIcon } from "../shared/FileIcon";

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
  focusIndex = -1,
}: ColumnViewProps) {
  return (
    <div data-testid="column-view" className="flex h-full overflow-x-auto">
      <div className="min-w-[250px] border-r border-[var(--color-border)]">
        {entries.map((entry, idx) => (
          <div
            key={entry.path}
            role="listitem"
            className={`flex cursor-pointer items-center justify-between px-3 py-1.5 hover:bg-[var(--color-bg-secondary)] ${
              selectedPaths.has(entry.path) ? "bg-[var(--color-accent)]/10" : ""
            } ${idx === focusIndex ? "outline outline-2 outline-[var(--color-accent)]" : ""}`}
            onClick={() => onSelect(entry)}
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
    </div>
  );
}
