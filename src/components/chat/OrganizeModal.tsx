import { useEffect, useCallback } from "react";
import { useChatStore } from "../../stores/chatStore";
import { useChat } from "../../hooks/useChat";
import { OrganizeCard } from "./OrganizeCard";
import type { FileAction } from "../../utils/actionParser";
import type { OrganizeProgressPhase } from "../../stores/chatStore";

function phaseLabel(phase: OrganizeProgressPhase): string {
  switch (phase) {
    case "indexing":
      return "Indexing";
    case "planning":
      return "Planning";
    case "applying":
      return "Applying";
    case "done":
      return "Ready";
    case "cancelled":
      return "Cancelled";
    case "error":
      return "Failed";
    default:
      return "Organizing";
  }
}

export function OrganizeModal() {
  const organize = useChatStore((s) => s.organize);
  const resetOrganize = useChatStore((s) => s.resetOrganize);
  const { executeOrganize, applyOrganize, cancelActiveOrganize } = useChat();

  const handleApprovePlan = useCallback(
    (filteredPlanRaw: string) => {
      const { organize: org } = useChatStore.getState();
      if (org.plan && org.folderPath) {
        executeOrganize(org.folderPath, filteredPlanRaw);
      }
    },
    [executeOrganize],
  );

  const handleApproveAll = useCallback(
    async (actions: FileAction[]) => {
      const { organize: org } = useChatStore.getState();
      if (!org.folderPath || !org.planRaw) return;
      if (actions.length === 0) return;
      await applyOrganize(org.folderPath, org.planRaw);
    },
    [applyOrganize],
  );

  useEffect(() => {
    if (organize.phase === "idle") return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") resetOrganize();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [organize.phase, resetOrganize]);

  if (organize.phase === "idle") return null;

  const progress = organize.progress;
  const combinedPercent = Math.max(0, Math.min(100, Math.round(progress?.combinedPercent ?? 0)));
  const isRunning = progress
    ? progress.phase === "indexing" ||
      progress.phase === "planning" ||
      progress.phase === "applying"
    : false;
  const showCancel = isRunning && progress?.phase !== "done";
  const cardReturnsNull = organize.phase === "planning" || organize.phase === "executing";

  return (
    <div
      data-testid="organize-modal"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={(e) => {
        if (e.target === e.currentTarget) resetOrganize();
      }}
    >
      <div
        className="w-full max-w-[560px] rounded-xl bg-[var(--color-bg)] p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {progress && (
          <div
            className={`mb-4 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-3 ${cardReturnsNull ? "flex flex-col justify-center" : ""}`}
          >
            <div className="flex items-center justify-between gap-3">
              <p className="text-sm font-semibold text-[var(--color-text)]">
                {phaseLabel(progress.phase)} {isRunning && `${combinedPercent}%`}
              </p>
              {showCancel && (
                <button
                  onClick={cancelActiveOrganize}
                  className="rounded-md px-2 py-1 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]"
                >
                  Cancel
                </button>
              )}
            </div>
            <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-[var(--color-border)]">
              <div
                className="h-full rounded-full transition-[width] duration-300 ease-out"
                style={{
                  width: `${combinedPercent}%`,
                  backgroundImage: "linear-gradient(90deg, var(--color-accent), #22c55e)",
                }}
              />
            </div>
            {progress.message && (
              <p className="mt-1.5 text-xs text-[var(--color-text-secondary)]">
                {progress.message}
              </p>
            )}
          </div>
        )}
        <OrganizeCard
          folderPath={organize.folderPath}
          phase={organize.phase}
          plan={organize.plan}
          executeContent={organize.executeContent}
          error={organize.error}
          onApprovePlan={handleApprovePlan}
          onCancelPlan={resetOrganize}
          onApproveAll={handleApproveAll}
          onDenyAll={resetOrganize}
        />
      </div>
    </div>
  );
}
