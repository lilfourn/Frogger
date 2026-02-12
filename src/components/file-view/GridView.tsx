import type { FileEntry } from "../../types/file";
import { FileIcon } from "../shared/FileIcon";

interface GridViewProps {
  entries: FileEntry[];
  onSelect: (entry: FileEntry) => void;
  onOpen: (entry: FileEntry) => void;
  onItemContextMenu: (e: React.MouseEvent, entry: FileEntry) => void;
  selectedPaths: Set<string>;
  focusIndex?: number;
}

export function GridView({
  entries,
  onSelect,
  onOpen,
  onItemContextMenu,
  selectedPaths,
  focusIndex = -1,
}: GridViewProps) {
  return (
    <div
      data-testid="grid-view"
      className="grid grid-cols-[repeat(auto-fill,minmax(100px,1fr))] gap-2 p-2"
    >
      {entries.map((entry, idx) => (
        <div
          key={entry.path}
          role="listitem"
          className={`flex cursor-pointer flex-col items-center gap-1 rounded p-2 hover:bg-[var(--color-bg-secondary)] ${
            selectedPaths.has(entry.path) ? "bg-[var(--color-accent)]/10" : ""
          } ${idx === focusIndex ? "outline outline-2 outline-[var(--color-accent)]" : ""}`}
          onClick={() => onSelect(entry)}
          onDoubleClick={() => onOpen(entry)}
          onContextMenu={(e) => onItemContextMenu(e, entry)}
        >
          <FileIcon isDirectory={entry.is_directory} size={40} />
          <span className="w-full truncate text-center text-xs">{entry.name}</span>
        </div>
      ))}
    </div>
  );
}
