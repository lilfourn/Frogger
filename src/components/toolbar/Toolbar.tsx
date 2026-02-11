import {
  List,
  LayoutGrid,
  Columns3,
  GalleryHorizontalEnd,
  ChevronUp,
  ChevronDown,
} from "lucide-react";
import { useSettingsStore, type ViewMode } from "../../stores/settingsStore";
import { useFileStore, type SortField } from "../../stores/fileStore";

const VIEW_OPTIONS: { mode: ViewMode; label: string; Icon: typeof List }[] = [
  { mode: "list", label: "List view", Icon: List },
  { mode: "grid", label: "Grid view", Icon: LayoutGrid },
  { mode: "column", label: "Column view", Icon: Columns3 },
  { mode: "gallery", label: "Gallery view", Icon: GalleryHorizontalEnd },
];

const BUTTON_SIZE = 28;

export function Toolbar() {
  const viewMode = useSettingsStore((s) => s.viewMode);
  const setViewMode = useSettingsStore((s) => s.setViewMode);
  const sortBy = useFileStore((s) => s.sortBy);
  const sortDirection = useFileStore((s) => s.sortDirection);
  const setSortBy = useFileStore((s) => s.setSortBy);
  const toggleSortDirection = useFileStore((s) => s.toggleSortDirection);

  const activeIndex = VIEW_OPTIONS.findIndex((o) => o.mode === viewMode);

  return (
    <div className="flex items-center gap-3 border-b border-[var(--color-border)] px-3 py-1.5">
      <div
        className="relative flex rounded-full bg-[var(--color-border)] p-0.5"
        role="radiogroup"
        aria-label="View mode"
      >
        <div
          className="absolute top-0.5 left-0.5 rounded-full bg-[var(--color-accent)] transition-transform duration-200 ease-out"
          style={{
            width: BUTTON_SIZE,
            height: BUTTON_SIZE,
            transform: `translateX(${activeIndex * BUTTON_SIZE}px)`,
          }}
        />
        {VIEW_OPTIONS.map(({ mode, label, Icon }) => (
          <button
            key={mode}
            role="radio"
            aria-checked={viewMode === mode}
            aria-label={label}
            onClick={() => setViewMode(mode)}
            className="relative z-10 flex items-center justify-center rounded-full"
            style={{ width: BUTTON_SIZE, height: BUTTON_SIZE }}
          >
            <Icon
              size={14}
              strokeWidth={1.5}
              className={viewMode === mode ? "text-white" : "text-[var(--color-text-secondary)]"}
            />
          </button>
        ))}
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
          className="flex h-7 w-7 items-center justify-center rounded text-xs hover:bg-[var(--color-border)]"
        >
          {sortDirection === "asc" ? (
            <ChevronUp size={14} strokeWidth={1.5} />
          ) : (
            <ChevronDown size={14} strokeWidth={1.5} />
          )}
        </button>
      </div>
    </div>
  );
}
