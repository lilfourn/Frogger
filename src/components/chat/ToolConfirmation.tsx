import { useState } from "react";
import { AlertTriangle, Check, X, File } from "lucide-react";
import type { FileAction } from "../../utils/actionParser";
import { describeAction, getActionFilePaths } from "../../utils/actionParser";

interface ToolConfirmationProps {
  action: FileAction;
  onApprove: (action: FileAction) => void;
  onDeny: (action: FileAction) => void;
}

export function ToolConfirmation({ action, onApprove, onDeny }: ToolConfirmationProps) {
  const [status, setStatus] = useState<"pending" | "approved" | "denied" | "running">("pending");
  const description = describeAction(action);
  const filePaths = getActionFilePaths(action);

  async function handleApprove() {
    setStatus("running");
    try {
      await onApprove(action);
      setStatus("approved");
    } catch (err) {
      console.error("[Chat] Action execution failed:", err);
      setStatus("pending");
    }
  }

  function handleDeny() {
    setStatus("denied");
    onDeny(action);
  }

  return (
    <div
      data-testid="tool-confirmation"
      className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-3 py-2"
    >
      <div className="flex items-center gap-2 text-xs font-semibold text-[var(--color-text)]">
        <AlertTriangle size={14} strokeWidth={1.5} className="text-[var(--color-accent)]" />
        File Operation
      </div>

      <p className="mt-1 text-sm text-[var(--color-text-secondary)]">{description}</p>

      {filePaths.length > 0 && (
        <div className="mt-1 space-y-0.5">
          {filePaths.map((path) => (
            <div
              key={path}
              className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]"
            >
              <File size={10} strokeWidth={1.5} />
              <span className="truncate">{path.split("/").pop()}</span>
            </div>
          ))}
        </div>
      )}

      {status === "pending" && (
        <div className="mt-2 flex items-center gap-2">
          <button
            data-testid="tool-approve"
            onClick={handleApprove}
            className="flex items-center gap-1 rounded bg-[var(--color-accent)] px-3 py-1 text-xs text-white"
          >
            <Check size={12} strokeWidth={2} /> Approve
          </button>
          <button
            data-testid="tool-deny"
            onClick={handleDeny}
            className="rounded px-3 py-1 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]"
          >
            Deny
          </button>
        </div>
      )}

      {status === "running" && (
        <div className="mt-2 flex items-center gap-1.5 text-xs text-[var(--color-text-secondary)]">
          <svg className="h-3 w-3 animate-spin" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="2" opacity="0.3" />
            <path d="M14 8a6 6 0 0 0-6-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          Running...
        </div>
      )}

      {status === "approved" && (
        <div className="mt-2 flex items-center gap-1.5 text-xs text-[var(--color-accent)]">
          <Check size={12} strokeWidth={2} /> Done
        </div>
      )}

      {status === "denied" && (
        <div className="mt-2 flex items-center gap-1.5 text-xs text-[var(--color-text-secondary)]">
          <X size={12} strokeWidth={2} /> Denied
        </div>
      )}
    </div>
  );
}
