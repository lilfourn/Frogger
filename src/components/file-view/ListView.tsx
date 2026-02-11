import type { FileEntry } from "../../types/file";
import { useFileStore } from "../../stores/fileStore";
import { FileIcon } from "../shared/FileIcon";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function formatDate(iso: string | null): string {
  if (!iso) return "\u2014";
  const d = new Date(iso);
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function getKind(entry: FileEntry): string {
  if (entry.is_directory) return "Folder";
  return entry.extension?.toUpperCase() ?? "File";
}

interface ListViewProps {
  entries: FileEntry[];
  onNavigate: (entry: FileEntry) => void;
}

export function ListView({ entries, onNavigate }: ListViewProps) {
  const sortBy = useFileStore((s) => s.sortBy);
  const sortDirection = useFileStore((s) => s.sortDirection);
  const setSortBy = useFileStore((s) => s.setSortBy);
  const toggleSortDirection = useFileStore((s) => s.toggleSortDirection);

  function handleColumnClick(col: "name" | "size" | "date" | "kind") {
    if (sortBy === col) {
      toggleSortDirection();
    } else {
      setSortBy(col);
    }
  }

  const arrow = sortDirection === "asc" ? "\u25B2" : "\u25BC";

  return (
    <div data-testid="list-view" className="w-full">
      <div className="sticky top-0 flex border-b border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-xs font-semibold text-[var(--color-text-secondary)]">
        <button className="flex-1 text-left" onClick={() => handleColumnClick("name")}>
          Name {sortBy === "name" && arrow}
        </button>
        <button className="w-20 text-right" onClick={() => handleColumnClick("size")}>
          Size {sortBy === "size" && arrow}
        </button>
        <button className="w-28 text-right" onClick={() => handleColumnClick("date")}>
          Modified {sortBy === "date" && arrow}
        </button>
        <button className="w-20 text-right" onClick={() => handleColumnClick("kind")}>
          Kind {sortBy === "kind" && arrow}
        </button>
      </div>

      <div className="overflow-auto">
        {entries.map((entry) => (
          <div
            key={entry.path}
            className="flex cursor-pointer items-center px-3 py-1.5 hover:bg-[var(--color-bg-secondary)]"
            onClick={() => onNavigate(entry)}
          >
            <div className="flex flex-1 items-center gap-2 overflow-hidden">
              <FileIcon isDirectory={entry.is_directory} size={16} />
              <span className="truncate text-sm">{entry.name}</span>
            </div>
            <span className="w-20 text-right text-xs text-[var(--color-text-secondary)]">
              {entry.is_directory ? "\u2014" : formatSize(entry.size_bytes ?? 0)}
            </span>
            <span className="w-28 text-right text-xs text-[var(--color-text-secondary)]">
              {formatDate(entry.modified_at)}
            </span>
            <span className="w-20 text-right text-xs text-[var(--color-text-secondary)]">
              {getKind(entry)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
