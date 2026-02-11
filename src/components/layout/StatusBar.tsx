import appLogo from "../../assets/app-logo.svg";

interface StatusBarProps {
  itemCount: number;
  currentPath?: string;
}

export function StatusBar({ itemCount }: StatusBarProps) {
  return (
    <div
      data-testid="status-bar"
      className="flex items-center justify-between border-t border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-4 py-1"
    >
      <span className="text-xs text-[var(--color-text-secondary)]">
        {itemCount} {itemCount === 1 ? "item" : "items"}
      </span>
      <div className="flex items-center gap-2">
        <img src={appLogo} alt="Frogger" width={16} height={16} className="shrink-0 opacity-60" />
        <span className="text-xs font-medium text-[var(--color-text-secondary)] opacity-60">
          Frogger
        </span>
      </div>
    </div>
  );
}
