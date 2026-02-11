import { useFileStore } from "../../stores/fileStore";

function tabLabel(path: string): string {
  if (!path) return "New Tab";
  const segments = path.replace(/\/+$/, "").split("/");
  return segments[segments.length - 1] || "/";
}

export function TabBar() {
  const tabs = useFileStore((s) => s.tabs);
  const activeTabId = useFileStore((s) => s.activeTabId);
  const switchTab = useFileStore((s) => s.switchTab);
  const closeTab = useFileStore((s) => s.closeTab);
  const addTab = useFileStore((s) => s.addTab);

  return (
    <div className="flex items-center overflow-x-auto border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          className={`group flex shrink-0 items-center gap-1 border-r border-[var(--color-border)] px-3 py-1.5 text-xs ${
            tab.id === activeTabId
              ? "bg-[var(--color-bg)] text-[var(--color-text)]"
              : "text-[var(--color-text-secondary)] hover:bg-[var(--color-bg)]"
          }`}
          onClick={() => switchTab(tab.id)}
          onAuxClick={(e) => {
            if (e.button === 1) closeTab(tab.id);
          }}
        >
          <span className="max-w-[120px] truncate">{tabLabel(tab.path)}</span>
          {tabs.length > 1 && (
            <span
              role="button"
              aria-label="Close tab"
              className="ml-1 rounded px-0.5 opacity-0 hover:bg-[var(--color-border)] group-hover:opacity-100"
              onClick={(e) => {
                e.stopPropagation();
                closeTab(tab.id);
              }}
            >
              &times;
            </span>
          )}
        </button>
      ))}
      <button
        aria-label="New tab"
        onClick={addTab}
        className="shrink-0 px-2 py-1.5 text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
      >
        +
      </button>
    </div>
  );
}
