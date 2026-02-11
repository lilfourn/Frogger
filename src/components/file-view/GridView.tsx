import type { FileEntry } from "../../types/file";
import { FileIcon } from "../shared/FileIcon";

interface GridViewProps {
  entries: FileEntry[];
  onNavigate: (entry: FileEntry) => void;
  focusIndex?: number;
}

export function GridView({ entries, onNavigate, focusIndex = -1 }: GridViewProps) {
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
            idx === focusIndex ? "outline outline-2 outline-[var(--color-accent)]" : ""
          }`}
          onClick={() => onNavigate(entry)}
        >
          <FileIcon isDirectory={entry.is_directory} size={40} />
          <span className="w-full truncate text-center text-xs">{entry.name}</span>
        </div>
      ))}
    </div>
  );
}
