import { useCallback } from "react";
import { Check } from "lucide-react";
import { useChatStore } from "../../stores/chatStore";
import { useChat } from "../../hooks/useChat";
import { OrganizeCard } from "./OrganizeCard";
import type { FileAction } from "../../utils/actionParser";
import type { OrganizeProgressPhase } from "../../stores/chatStore";

function phaseLabel(phase: OrganizeProgressPhase): string {
  switch (phase) {
    case "indexing":
      return "Indexing files...";
    case "planning":
      return "Planning organization...";
    case "applying":
      return "Applying changes...";
    case "done":
      return "Complete";
    case "cancelled":
      return "Cancelled";
    case "error":
      return "Error";
    default:
      return "Organizing";
  }
}

const PHASES = ["indexing", "planning", "applying"] as const;
const PHASE_LABELS = { indexing: "Index", planning: "Plan", applying: "Apply" };

function phaseIndex(phase: OrganizeProgressPhase): number {
  const idx = PHASES.indexOf(phase as (typeof PHASES)[number]);
  return idx === -1 ? PHASES.length : idx;
}

function PhaseSteps({ phase }: { phase: OrganizeProgressPhase }) {
  const active = phaseIndex(phase);
  return (
    <div className="mb-3 flex items-center justify-center gap-0">
      {PHASES.map((p, i) => {
        const completed = i < active;
        const isCurrent = i === active;
        return (
          <div key={p} className="flex items-center">
            {i > 0 && (
              <div
                className="h-px w-6"
                style={{ backgroundColor: completed ? "var(--color-accent)" : "var(--color-border)" }}
              />
            )}
            <div className="flex flex-col items-center gap-1">
              <div
                className="flex h-5 w-5 items-center justify-center rounded-full text-[10px] font-bold"
                style={
                  completed
                    ? { backgroundColor: "var(--color-accent)", color: "#fff" }
                    : isCurrent
                      ? { border: "2px solid var(--color-accent)", color: "var(--color-accent)" }
                      : { border: "2px solid var(--color-border)", color: "var(--color-text-secondary)" }
                }
              >
                {completed ? <Check size={12} strokeWidth={3} /> : i + 1}
              </div>
              <span
                className="text-[10px] font-medium"
                style={{ color: completed || isCurrent ? "var(--color-text)" : "var(--color-text-secondary)" }}
              >
                {PHASE_LABELS[p]}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

export function OrganizeModal() {
  const organize = useChatStore((s) => s.organize);
  const { executeOrganize, applyOrganize, cancelActiveOrganize, retryOrganize, openOrganizePath } =
    useChat();

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

  if (organize.phase === "idle") return null;

  const progress = organize.progress;
  const combinedPercent = Math.max(0, Math.min(100, Math.round(progress?.combinedPercent ?? 0)));
  const phasePercent = Math.max(0, Math.min(100, Math.round(progress?.percent ?? 0)));
  const isRunning = progress
    ? progress.phase === "indexing" ||
      progress.phase === "planning" ||
      progress.phase === "applying"
    : false;
  const showCancel = isRunning && progress?.phase !== "done";
  const cardReturnsNull = organize.phase === "planning" || organize.phase === "executing";
  const loadingOnly = Boolean(progress) && cardReturnsNull;

  return (
    <div
      data-testid="organize-modal"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div
        className={`w-full max-w-[560px] rounded-xl bg-[var(--color-bg)] p-5 shadow-2xl ${loadingOnly ? "flex min-h-[220px] flex-col justify-center" : ""}`}
      >
        {progress && (
          <div
            data-testid="organize-progress-shell"
            className={`w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-3 ${loadingOnly ? "" : "mb-4"}`}
          >
            <PhaseSteps phase={progress.phase} />
            <div className="h-2 w-full overflow-hidden rounded-full bg-[var(--color-border)]">
              <div
                data-testid="organize-progress-bar"
                className="h-full rounded-full transition-[width] duration-300 ease-out"
                style={{
                  width: `${combinedPercent}%`,
                  backgroundImage: isRunning
                    ? "linear-gradient(90deg, transparent, rgba(255,255,255,0.3), transparent), linear-gradient(90deg, var(--color-accent), #22c55e)"
                    : "linear-gradient(90deg, var(--color-accent), #22c55e)",
                  backgroundSize: isRunning ? "200% 100%, 100% 100%" : undefined,
                  animation: isRunning ? "progress-shimmer 1.5s linear infinite" : undefined,
                }}
              />
            </div>
            <div className="mt-2 flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-sm font-semibold text-[var(--color-text)]">
                  {phaseLabel(progress.phase)} {isRunning && `${phasePercent}%`}
                </p>
                {progress.message && (
                  <p className="mt-0.5 text-xs text-[var(--color-text-secondary)]">
                    {progress.message}
                  </p>
                )}
              </div>
              {showCancel && (
                <button
                  onClick={cancelActiveOrganize}
                  className="shrink-0 rounded-md px-2 py-1 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]"
                >
                  Cancel
                </button>
              )}
            </div>
          </div>
        )}
        <OrganizeCard
          folderPath={organize.folderPath}
          phase={organize.phase}
          plan={organize.plan}
          executeContent={organize.executeContent}
          error={organize.error}
          onApprovePlan={handleApprovePlan}
          onCancelPlan={cancelActiveOrganize}
          onApproveAll={handleApproveAll}
          onDenyAll={cancelActiveOrganize}
          onOpenPath={openOrganizePath}
          onRetry={retryOrganize}
        />
      </div>
    </div>
  );
}
