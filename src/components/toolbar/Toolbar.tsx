import { useSettingsStore } from "../../stores/settingsStore";
import { useFileStore, type SortField } from "../../stores/fileStore";

export function Toolbar() {
  const viewMode = useSettingsStore((s) => s.viewMode);
  const setViewMode = useSettingsStore((s) => s.setViewMode);
  const sortBy = useFileStore((s) => s.sortBy);
  const sortDirection = useFileStore((s) => s.sortDirection);
  const setSortBy = useFileStore((s) => s.setSortBy);
  const toggleSortDirection = useFileStore((s) => s.toggleSortDirection);

  return (
    <div className="flex items-center gap-3 border-b border-[var(--color-border)] px-3 py-1.5">
      <div className="flex gap-1">
        <button
          aria-label="List view"
          onClick={() => setViewMode("list")}
          className={`rounded p-1.5 text-xs ${
            viewMode === "list"
              ? "bg-[var(--color-accent)] text-white"
              : "hover:bg-[var(--color-border)]"
          }`}
        >
          &#9776;
        </button>
        <button
          aria-label="Grid view"
          onClick={() => setViewMode("grid")}
          className={`rounded p-1.5 text-xs ${
            viewMode === "grid"
              ? "bg-[var(--color-accent)] text-white"
              : "hover:bg-[var(--color-border)]"
          }`}
        >
          &#9638;
        </button>
        <button
          aria-label="Column view"
          onClick={() => setViewMode("column")}
          className={`rounded p-1.5 text-xs ${
            viewMode === "column"
              ? "bg-[var(--color-accent)] text-white"
              : "hover:bg-[var(--color-border)]"
          }`}
        >
          &#9707;
        </button>
        <button
          aria-label="Gallery view"
          onClick={() => setViewMode("gallery")}
          className={`rounded p-1.5 text-xs ${
            viewMode === "gallery"
              ? "bg-[var(--color-accent)] text-white"
              : "hover:bg-[var(--color-border)]"
          }`}
        >
          &#9636;
        </button>
      </div>

      <div className="flex items-center gap-1">
        <select
          aria-label="Sort by"
          value={sortBy}
          onChange={(e) => setSortBy(e.target.value as SortField)}
          className="rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-xs text-[var(--color-text)]"
        >
          <option value="name">Name</option>
          <option value="size">Size</option>
          <option value="date">Date</option>
          <option value="kind">Kind</option>
        </select>
        <button
          aria-label="Toggle sort direction"
          onClick={toggleSortDirection}
          className="rounded p-1.5 text-xs hover:bg-[var(--color-border)]"
        >
          {sortDirection === "asc" ? "\u25B2" : "\u25BC"}
        </button>
      </div>
    </div>
  );
}
