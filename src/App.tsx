import { useEffect, useState } from "react";
import { listDirectory } from "./services/fileService";
import type { FileEntry } from "./types/file";

const DEFAULT_PATH = navigator.userAgent.includes("Windows") ? "C:\\Users" : "/Users";

function App() {
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [currentPath, setCurrentPath] = useState<string>(DEFAULT_PATH);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listDirectory(currentPath)
      .then((result) => {
        if (!cancelled) setEntries(result);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [currentPath]);

  function handleEntryClick(entry: FileEntry) {
    if (entry.is_directory) {
      setError(null);
      setCurrentPath(entry.path);
    }
  }

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-[var(--color-border)] px-4 py-2 text-sm text-[var(--color-text-secondary)]">
        {currentPath}
      </div>
      <div className="flex-1 overflow-auto p-2">
        {error && <div className="p-4 text-red-500">{error}</div>}
        {entries.map((entry) => (
          <div
            key={entry.path}
            className="flex cursor-pointer items-center gap-2 rounded px-3 py-1.5 hover:bg-[var(--color-bg-secondary)]"
            onClick={() => handleEntryClick(entry)}
          >
            <span className="text-sm">{entry.is_directory ? "📁" : "📄"}</span>
            <span className="flex-1 truncate text-sm">{entry.name}</span>
            {entry.size_bytes != null && !entry.is_directory && (
              <span className="text-xs text-[var(--color-text-secondary)]">
                {formatSize(entry.size_bytes)}
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export default App;
