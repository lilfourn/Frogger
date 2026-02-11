import type { FileEntry } from "../../types/file";
import { FileIcon } from "../shared/FileIcon";

interface ColumnViewProps {
  entries: FileEntry[];
  onNavigate: (entry: FileEntry) => void;
  focusIndex?: number;
}

export function ColumnView({ entries, onNavigate, focusIndex = -1 }: ColumnViewProps) {
  return (
    <div data-testid="column-view" className="flex h-full overflow-x-auto">
      <div className="min-w-[250px] border-r border-[var(--color-border)]">
        {entries.map((entry, idx) => (
          <div
            key={entry.path}
            role="listitem"
            className={`flex cursor-pointer items-center justify-between px-3 py-1.5 hover:bg-[var(--color-bg-secondary)] ${
              idx === focusIndex
                ? "bg-[var(--color-accent)]/10 outline outline-2 outline-[var(--color-accent)]"
                : ""
            }`}
            onClick={() => onNavigate(entry)}
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
