interface ProgressBarProps {
  label: string;
  progress: number;
  onCancel: () => void;
  visible?: boolean;
}

export function ProgressBar({ label, progress, onCancel, visible = true }: ProgressBarProps) {
  if (!visible) return null;

  const clamped = Math.max(0, Math.min(100, Math.round(progress)));

  return (
    <div className="border-t border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-4 py-2">
      <div className="flex items-center justify-between text-xs">
        <span className="text-[var(--color-text-secondary)]">{label}</span>
        <div className="flex items-center gap-2">
          <span className="text-[var(--color-text-secondary)]">{clamped}%</span>
          <button
            onClick={onCancel}
            aria-label="Cancel"
            className="rounded px-2 py-0.5 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]"
          >
            Cancel
          </button>
        </div>
      </div>
      <div
        role="progressbar"
        aria-valuenow={clamped}
        aria-valuemin={0}
        aria-valuemax={100}
        className="mt-1 h-1.5 w-full overflow-hidden rounded-full bg-[var(--color-border)]"
      >
        <div
          className="h-full rounded-full bg-[var(--color-accent)] transition-[width] duration-150"
          style={{ width: `${clamped}%` }}
        />
      </div>
    </div>
  );
}
