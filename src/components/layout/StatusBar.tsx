interface StatusBarProps {
  itemCount: number;
  currentPath?: string;
}

export function StatusBar({ itemCount, currentPath }: StatusBarProps) {
  return (
    <div
      data-testid="status-bar"
      className="flex items-center justify-between border-t border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-4 py-1"
    >
      <span className="text-xs text-[var(--color-text-secondary)]">
        {itemCount} {itemCount === 1 ? "item" : "items"}
      </span>
      {currentPath && (
        <span className="truncate text-xs text-[var(--color-text-secondary)]">
          {currentPath}
        </span>
      )}
    </div>
  );
}
