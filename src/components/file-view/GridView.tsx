import type { FileEntry } from "../../types/file";
import { FileIcon } from "../shared/FileIcon";

interface GridViewProps {
  entries: FileEntry[];
  onNavigate: (entry: FileEntry) => void;
}

export function GridView({ entries, onNavigate }: GridViewProps) {
  return (
    <div
      data-testid="grid-view"
      className="grid grid-cols-[repeat(auto-fill,minmax(100px,1fr))] gap-2 p-2"
    >
      {entries.map((entry) => (
        <div
          key={entry.path}
          className="flex cursor-pointer flex-col items-center gap-1 rounded p-2 hover:bg-[var(--color-bg-secondary)]"
          onClick={() => onNavigate(entry)}
        >
          <FileIcon isDirectory={entry.is_directory} size={40} />
          <span className="w-full truncate text-center text-xs">{entry.name}</span>
        </div>
      ))}
    </div>
  );
}
