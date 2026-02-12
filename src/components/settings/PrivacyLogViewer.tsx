import { useState, useEffect } from "react";
import { getAuditLog, type AuditLogEntry } from "../../services/settingsService";

export function PrivacyLogViewer() {
  const [entries, setEntries] = useState<AuditLogEntry[]>([]);

  useEffect(() => {
    getAuditLog(100).then(setEntries).catch((err) => console.error("[Privacy] Failed to load audit log:", err));
  }, []);

  if (entries.length === 0) {
    return (
      <p data-testid="audit-log-empty" className="text-xs text-[var(--color-text-secondary)]">
        No API calls logged yet.
      </p>
    );
  }

  return (
    <div data-testid="audit-log" className="max-h-[300px] space-y-1.5 overflow-y-auto">
      {entries.map((entry) => (
        <div
          key={entry.id}
          className="rounded border border-[var(--color-border)] px-3 py-1.5"
        >
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-[var(--color-text)]">
              {entry.endpoint}
            </span>
            <span className="text-[10px] text-[var(--color-text-secondary)]">
              {new Date(entry.created_at).toLocaleString()}
            </span>
          </div>
          {entry.request_summary && (
            <p className="mt-0.5 text-xs text-[var(--color-text-secondary)]">
              {entry.request_summary}
            </p>
          )}
          {entry.tokens_used != null && (
            <span className="text-[10px] text-[var(--color-text-secondary)]">
              {entry.tokens_used} tokens
            </span>
          )}
        </div>
      ))}
    </div>
  );
}
