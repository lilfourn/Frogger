import { useState, useCallback } from "react";
import { Files, FileWarning, Copy, ChevronDown, ChevronRight } from "lucide-react";
import { useFileStore } from "../../stores/fileStore";
import { findLargeFiles, findOldFiles, findDuplicates } from "../../services/fileService";
import type { FileEntry } from "../../types/file";

type SmartCategory = "large" | "old" | "duplicates";

export function SmartFolders() {
  const currentPath = useFileStore((s) => s.currentPath);
  const [expanded, setExpanded] = useState(false);
  const [active, setActive] = useState<SmartCategory | null>(null);
  const [results, setResults] = useState<FileEntry[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(
    async (category: SmartCategory) => {
      if (active === category) {
        setActive(null);
        setResults([]);
        return;
      }
      setLoading(true);
      setActive(category);
      try {
        let entries: FileEntry[] = [];
        switch (category) {
          case "large":
            entries = await findLargeFiles(currentPath, 10 * 1024 * 1024);
            break;
          case "old":
            entries = await findOldFiles(currentPath, 365);
            break;
          case "duplicates": {
            const groups = await findDuplicates(currentPath);
            entries = groups.flat();
            break;
          }
        }
        setResults(entries);
      } catch (err) {
        console.error("[SmartFolders] Failed to load:", err);
        setResults([]);
      } finally {
        setLoading(false);
      }
    },
    [currentPath, active],
  );

  const categories: { key: SmartCategory; label: string; icon: typeof Files }[] = [
    { key: "large", label: "Large Files (>10MB)", icon: FileWarning },
    { key: "old", label: "Old Files (>1yr)", icon: Files },
    { key: "duplicates", label: "Duplicates", icon: Copy },
  ];

  return (
    <section data-testid="smart-folders">
      <button
        onClick={() => setExpanded(!expanded)}
        className="mb-1 flex w-full items-center gap-1 text-xs font-semibold uppercase text-[var(--color-text-secondary)]"
      >
        {expanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        Smart Folders
      </button>

      {expanded && (
        <div className="space-y-0.5">
          {categories.map(({ key, label, icon: Icon }) => (
            <div key={key}>
              <button
                onClick={() => load(key)}
                className={`flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-[var(--color-border)] ${active === key ? "bg-[var(--color-border)]" : ""}`}
              >
                <Icon size={15} strokeWidth={1.5} className="shrink-0" />
                {label}
              </button>
              {active === key && (
                <div className="ml-6 max-h-[150px] overflow-y-auto">
                  {loading && (
                    <span className="text-xs text-[var(--color-text-secondary)]">Loading...</span>
                  )}
                  {!loading && results.length === 0 && (
                    <span className="text-xs text-[var(--color-text-secondary)]">None found</span>
                  )}
                  {!loading &&
                    results.map((f) => (
                      <div
                        key={f.path}
                        className="truncate py-0.5 text-xs text-[var(--color-text-secondary)]"
                        title={f.path}
                      >
                        {f.name}
                        {f.size_bytes != null && (
                          <span className="ml-1 opacity-60">
                            ({(f.size_bytes / 1024 / 1024).toFixed(1)}MB)
                          </span>
                        )}
                      </div>
                    ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
