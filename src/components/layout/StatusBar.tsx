import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Settings } from "lucide-react";
import appLogo from "../../assets/app-logo.svg";
import { useTauriEvent } from "../../hooks/useTauriEvents";

interface StatusBarProps {
  itemCount: number;
  currentPath?: string;
  onSettingsClick?: () => void;
}

interface IndexingProgress {
  processed: number;
  total: number;
  status: string;
}

function shouldShowIndexing(progress: IndexingProgress | null): progress is IndexingProgress {
  if (!progress) {
    return false;
  }
  return progress.status !== "done" && progress.status !== "error";
}

function indexingMessage(progress: IndexingProgress): string {
  if (progress.status === "starting" || progress.total === 0) {
    return "Indexing...";
  }
  return `${progress.processed}/${progress.total} files indexed`;
}

export function StatusBar({ itemCount, onSettingsClick }: StatusBarProps) {
  const [progress, setProgress] = useState<IndexingProgress | null>(null);

  useEffect(() => {
    let active = true;

    invoke<IndexingProgress>("get_indexing_status")
      .then((payload) => {
        if (!active) return;
        setProgress(shouldShowIndexing(payload) ? payload : null);
      })
      .catch((err) => {
        console.error("[StatusBar] Failed to get indexing status:", err);
      });

    return () => {
      active = false;
    };
  }, []);

  useTauriEvent<IndexingProgress>("indexing-progress", (payload) => {
    setProgress(shouldShowIndexing(payload) ? payload : null);
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
            {indexingMessage(progress)}
          </span>
        )}
        {onSettingsClick && (
          <button
            data-testid="settings-btn"
            onClick={onSettingsClick}
            aria-label="Open settings"
            className="rounded p-0.5 text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]"
          >
            <Settings size={13} strokeWidth={1.5} />
          </button>
        )}
        <img src={appLogo} alt="Frogger" width={16} height={16} className="shrink-0 opacity-60" />
        <span className="text-xs font-medium text-[var(--color-text-secondary)] opacity-60">
          Frogger
        </span>
      </div>
    </div>
  );
}
