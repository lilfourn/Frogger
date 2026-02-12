import { useEffect, useRef, useCallback } from "react";
import { Search } from "lucide-react";
import { useSearchStore } from "../../stores/searchStore";
import { useFileStore } from "../../stores/fileStore";
import { useSearch } from "../../hooks/useSearch";
import type { SearchResult } from "../../types/search";

export function SearchBar() {
  useSearch();

  const isOpen = useSearchStore((s) => s.isOpen);
  const query = useSearchStore((s) => s.query);
  const results = useSearchStore((s) => s.results);
  const isSearching = useSearchStore((s) => s.isSearching);
  const selectedIndex = useSearchStore((s) => s.selectedIndex);
  const setQuery = useSearchStore((s) => s.setQuery);
  const setSelectedIndex = useSearchStore((s) => s.setSelectedIndex);
  const close = useSearchStore((s) => s.close);

  const navigateTo = useFileStore((s) => s.navigateTo);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) inputRef.current?.focus();
  }, [isOpen]);

  const selectResult = useCallback(
    (result: SearchResult) => {
      const targetPath = result.is_directory
        ? result.file_path
        : result.file_path.replace(/\/[^/]+$/, "") || "/";
      navigateTo(targetPath);
      close();
    },
    [navigateTo, close],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        close();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex(Math.min(selectedIndex + 1, results.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex(Math.max(selectedIndex - 1, 0));
      } else if (e.key === "Enter" && results[selectedIndex]) {
        selectResult(results[selectedIndex]);
      }
    },
    [close, selectedIndex, setSelectedIndex, results, selectResult],
  );

  if (!isOpen) return null;

  return (
    <div
      data-testid="search-overlay"
      className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div className="w-full max-w-[560px] rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] shadow-lg">
        <div className="flex items-center gap-3 px-4 py-3">
          <Search
            size={16}
            strokeWidth={1.5}
            className="shrink-0 text-[var(--color-text-secondary)]"
          />
          <input
            ref={inputRef}
            data-testid="search-input"
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search files..."
            className="w-full bg-transparent text-sm text-[var(--color-text)] outline-none placeholder:text-[var(--color-text-secondary)]"
          />
          {isSearching && (
            <span className="shrink-0 text-xs text-[var(--color-text-secondary)]">...</span>
          )}
        </div>

        {results.length > 0 && (
          <div className="border-t border-[var(--color-border)]">
            {results.map((result, i) => {
              const parent = result.file_path.replace(/\/[^/]+$/, "") || "/";
              return (
                <button
                  key={result.file_path}
                  data-testid={`search-result-${i}`}
                  className={`flex w-full items-center gap-3 px-4 py-2 text-left ${
                    i === selectedIndex
                      ? "bg-[var(--color-bg-secondary)]"
                      : "hover:bg-[var(--color-bg-secondary)]"
                  }`}
                  onClick={() => selectResult(result)}
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm text-[var(--color-text)]">
                      {result.file_name}
                    </div>
                    <div className="truncate text-xs text-[var(--color-text-secondary)]">
                      {parent}
                    </div>
                  </div>
                  {result.match_source === "vec" && (
                    <span className="shrink-0 text-[10px] text-[var(--color-accent)]">
                      semantic
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
