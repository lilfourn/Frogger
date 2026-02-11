import { useEffect, useState } from "react";
import { listDirectory } from "./services/fileService";
import type { FileEntry } from "./types/file";
import { AppLayout } from "./components/layout/AppLayout";
import { useTheme } from "./hooks/useTheme";

const DEFAULT_PATH = navigator.userAgent.includes("Windows") ? "C:\\Users" : "/Users";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function App() {
  useTheme();

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

  const sidebar = (
    <div className="p-3">
      <div className="mb-2 text-xs font-semibold uppercase text-[var(--color-text-secondary)]">
        Favorites
      </div>
      <div className="space-y-0.5 text-sm">
        {[
          { name: "Home", path: `/Users/${navigator.userAgent.includes("Windows") ? "" : ""}` },
          { name: "Desktop", path: `${DEFAULT_PATH}/Desktop` },
          { name: "Documents", path: `${DEFAULT_PATH}/Documents` },
          { name: "Downloads", path: `${DEFAULT_PATH}/Downloads` },
        ].map((item) => (
          <button
            key={item.name}
            onClick={() => {
              setError(null);
              setCurrentPath(item.path);
            }}
            className="w-full rounded px-2 py-1 text-left hover:bg-[var(--color-border)]"
          >
            {item.name}
          </button>
        ))}
      </div>
    </div>
  );

  const main = (
    <div className="p-2">
      {error && <div className="p-4 text-red-500">{error}</div>}
      {entries.map((entry) => (
        <div
          key={entry.path}
          className="flex cursor-pointer items-center gap-2 rounded px-3 py-1.5 hover:bg-[var(--color-bg-secondary)]"
          onClick={() => handleEntryClick(entry)}
        >
          <span className="text-sm">{entry.is_directory ? "\uD83D\uDCC1" : "\uD83D\uDCC4"}</span>
          <span className="flex-1 truncate text-sm">{entry.name}</span>
          {entry.size_bytes != null && !entry.is_directory && (
            <span className="text-xs text-[var(--color-text-secondary)]">
              {formatSize(entry.size_bytes)}
            </span>
          )}
        </div>
      ))}
    </div>
  );

  return (
    <AppLayout sidebar={sidebar} main={main} itemCount={entries.length} currentPath={currentPath} />
  );
}

export default App;
