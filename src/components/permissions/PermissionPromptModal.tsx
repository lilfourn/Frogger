import { useEffect } from "react";
import { ShieldAlert } from "lucide-react";
import { usePermissionPromptStore } from "../../stores/permissionPromptStore";

function capabilityLabel(capability: string): string {
  switch (capability) {
    case "content_scan":
      return "Content scan";
    case "modification":
      return "Modification";
    case "ocr":
      return "OCR";
    case "indexing":
      return "Indexing";
    default:
      return capability.replace(/_/g, " ");
  }
}

export function PermissionPromptModal() {
  const queue = usePermissionPromptStore((s) => s.queue);
  const resolveCurrent = usePermissionPromptStore((s) => s.resolveCurrent);
  const cancelAll = usePermissionPromptStore((s) => s.cancelAll);
  const current = queue[0];

  useEffect(() => {
    return () => {
      if (document.hidden) cancelAll();
    };
  }, [cancelAll]);

  if (!current) {
    return null;
  }

  const blockedPreview = current.blocked.slice(0, 4);
  const hiddenCount = Math.max(current.blocked.length - blockedPreview.length, 0);

  return (
    <div
      data-testid="permission-prompt-overlay"
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/45 px-4"
    >
      <div
        data-testid="permission-prompt-modal"
        role="dialog"
        aria-modal="true"
        className="w-full max-w-[520px] rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] shadow-2xl"
      >
        <div className="flex items-start gap-3 border-b border-[var(--color-border)] px-4 py-3">
          <div className="mt-0.5 rounded-full bg-[var(--color-bg-secondary)] p-1.5">
            <ShieldAlert size={14} strokeWidth={1.75} className="text-[var(--color-accent)]" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-[var(--color-text)]">Permission required</h3>
            <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">{current.title}</p>
          </div>
        </div>

        {blockedPreview.length > 0 && (
          <div className="space-y-1 px-4 py-3">
            {blockedPreview.map((item, index) => (
              <div
                key={`${item.path}-${item.capability}-${index}`}
                className="text-xs text-[var(--color-text-secondary)]"
              >
                {capabilityLabel(item.capability)}: <span className="font-mono">{item.path}</span>
              </div>
            ))}
            {hiddenCount > 0 && (
              <p className="text-xs text-[var(--color-text-secondary)]">+{hiddenCount} more</p>
            )}
          </div>
        )}

        <div className="flex items-center justify-end gap-2 border-t border-[var(--color-border)] px-4 py-3">
          <button
            data-testid="permission-prompt-deny"
            onClick={() => resolveCurrent("deny")}
            className="rounded px-3 py-1.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
          >
            Deny
          </button>
          <button
            data-testid="permission-prompt-once"
            onClick={() => resolveCurrent("allow_once")}
            className="rounded border border-[var(--color-border)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
          >
            Allow once
          </button>
          {current.allowAlways && (
            <>
              {current.allowExactPath && (
                <button
                  data-testid="permission-prompt-always-exact"
                  onClick={() => resolveCurrent("always_allow_exact")}
                  className="rounded border border-[var(--color-border)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
                >
                  Always allow this exact path
                </button>
              )}
              <button
                data-testid="permission-prompt-always-folder"
                onClick={() => resolveCurrent("always_allow_folder")}
                className="rounded bg-[var(--color-accent)] px-3 py-1.5 text-sm text-white"
              >
                Always allow this folder
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
