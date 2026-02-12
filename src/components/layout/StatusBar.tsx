import { useState } from "react";
import appLogo from "../../assets/app-logo.svg";
import { useTauriEvent } from "../../hooks/useTauriEvents";

interface StatusBarProps {
  itemCount: number;
  currentPath?: string;
}

interface IndexingProgress {
  processed: number;
  total: number;
  status: string;
}

export function StatusBar({ itemCount }: StatusBarProps) {
  const [progress, setProgress] = useState<IndexingProgress | null>(null);

  useTauriEvent<IndexingProgress>("indexing-progress", (payload) => {
    setProgress(payload.status === "done" ? null : payload);
  });

  return (
    <div
      data-testid="status-bar"
      className="flex items-center justify-between border-t border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-4 py-1"
    >
      <span className="text-xs text-[var(--color-text-secondary)]">
        {itemCount} {itemCount === 1 ? "item" : "items"}
      </span>
      <div className="flex items-center gap-2">
        {progress && (
          <span
            data-testid="indexing-indicator"
            className="flex items-center gap-1.5 text-xs text-[var(--color-text-secondary)]"
          >
            <svg className="h-3 w-3 animate-spin" viewBox="0 0 16 16" fill="none">
              <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="2" opacity="0.3" />
              <path
                d="M14 8a6 6 0 0 0-6-6"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
            {progress.processed}/{progress.total} files indexed
          </span>
        )}
        <img src={appLogo} alt="Frogger" width={16} height={16} className="shrink-0 opacity-60" />
        <span className="text-xs font-medium text-[var(--color-text-secondary)] opacity-60">
          Frogger
        </span>
      </div>
    </div>
  );
}
