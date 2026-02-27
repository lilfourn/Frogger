import { List, LayoutGrid, Columns3, GalleryHorizontalEnd, ChevronRight } from "lucide-react";
import { useSettingsStore, type ViewMode } from "../../stores/settingsStore";
import { useFileStore } from "../../stores/fileStore";

const VIEW_OPTIONS: { mode: ViewMode; label: string; Icon: typeof List }[] = [
  { mode: "list", label: "List view", Icon: List },
  { mode: "grid", label: "Grid view", Icon: LayoutGrid },
  { mode: "column", label: "Column view", Icon: Columns3 },
  { mode: "gallery", label: "Gallery view", Icon: GalleryHorizontalEnd },
];

const BUTTON_SIZE = 28;

function pathSegments(path: string): { name: string; path: string }[] {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  if (!normalized) return [];

  const parts = normalized.split("/").filter(Boolean);
  if (parts.length === 0) return [];

  if (normalized.startsWith("/")) {
    return parts.map((name, i) => ({
      name,
      path: "/" + parts.slice(0, i + 1).join("/"),
    }));
  }

  return parts.map((name, i) => ({
    name,
    path: parts.slice(0, i + 1).join("/"),
  }));
}

export function Toolbar() {
  const viewMode = useSettingsStore((s) => s.viewMode);
  const setViewMode = useSettingsStore((s) => s.setViewMode);
  const currentPath = useFileStore((s) => s.currentPath);
  const navigateTo = useFileStore((s) => s.navigateTo);

  const activeIndex = VIEW_OPTIONS.findIndex((o) => o.mode === viewMode);
  const segments = pathSegments(currentPath);

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

      <nav aria-label="Path" className="flex min-w-0 flex-1 items-center overflow-hidden text-xs">
        <button
          onClick={() => navigateTo("/")}
          className="shrink-0 text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
        >
          /
        </button>
        {segments.map((seg, i) => (
          <span key={seg.path} className="flex items-center">
            <ChevronRight
              size={12}
              strokeWidth={1.5}
              className="mx-0.5 shrink-0 text-[var(--color-text-secondary)]"
            />
            {i === segments.length - 1 ? (
              <span className="truncate font-medium text-[var(--color-text)]">{seg.name}</span>
            ) : (
              <button
                onClick={() => navigateTo(seg.path)}
                className="truncate text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
              >
                {seg.name}
              </button>
            )}
          </span>
        ))}
      </nav>
    </div>
  );
}
