import { useEffect } from "react";
import { listDirectory } from "./services/fileService";
import { useFileStore } from "./stores/fileStore";
import { AppLayout } from "./components/layout/AppLayout";
import { Sidebar } from "./components/sidebar/Sidebar";
import { useTheme } from "./hooks/useTheme";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function App() {
  useTheme();

  const currentPath = useFileStore((s) => s.currentPath);
  const entries = useFileStore((s) => s.entries);
  const error = useFileStore((s) => s.error);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const setEntries = useFileStore((s) => s.setEntries);
  const setError = useFileStore((s) => s.setError);
  const setLoading = useFileStore((s) => s.setLoading);

  useEffect(() => {
    const defaultPath = navigator.userAgent.includes("Windows") ? "C:\\Users" : "/Users";
    if (!currentPath) navigateTo(defaultPath);
  }, [currentPath, navigateTo]);

  useEffect(() => {
    if (!currentPath) return;
    let cancelled = false;
    setLoading(true);
    listDirectory(currentPath)
      .then((result) => {
        if (!cancelled) {
          setEntries(result);
          setLoading(false);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [currentPath, setEntries, setError, setLoading]);

  const main = (
    <div className="p-2">
      {error && <div className="p-4 text-red-500">{error}</div>}
      {entries.map((entry) => (
        <div
          key={entry.path}
          className="flex cursor-pointer items-center gap-2 rounded px-3 py-1.5 hover:bg-[var(--color-bg-secondary)]"
          onClick={() => entry.is_directory && navigateTo(entry.path)}
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
    <AppLayout
      sidebar={<Sidebar />}
      main={main}
      itemCount={entries.length}
      currentPath={currentPath}
    />
  );
}

export default App;
