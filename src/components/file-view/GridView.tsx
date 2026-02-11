import type { FileEntry } from "../../types/file";

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
          <span className="text-3xl">
            {entry.is_directory ? "\uD83D\uDCC1" : "\uD83D\uDCC4"}
          </span>
          <span className="w-full truncate text-center text-xs">
            {entry.name}
          </span>
        </div>
      ))}
    </div>
  );
}
