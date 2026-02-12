import { FolderOpen, Check, AlertCircle, ChevronDown, ChevronRight, File } from "lucide-react";
import { useState } from "react";
import { parseActionBlocks } from "../../utils/actionParser";
import { DiffPreview } from "./DiffPreview";
import type { FileAction, OrganizePlan } from "../../utils/actionParser";
import type { OrganizePhase } from "../../stores/chatStore";

interface OrganizeCardProps {
  folderPath: string;
  phase: OrganizePhase;
  plan: OrganizePlan | null;
  executeContent: string;
  error: string;
  onApprovePlan: (filteredPlanRaw: string) => void;
  onCancelPlan: () => void;
  onApproveAll: (actions: FileAction[]) => void;
  onDenyAll: () => void;
}

interface PlanPreviewProps {
  plan: OrganizePlan;
  folderName: string;
  excludedFiles: Set<string>;
  onToggleFile: (filePath: string) => void;
}

function PlanPreview({ plan, folderName, excludedFiles, onToggleFile }: PlanPreviewProps) {
  const [expanded, setExpanded] = useState<Record<number, boolean>>({});

  const toggle = (i: number) => setExpanded((prev) => ({ ...prev, [i]: !prev[i] }));

  const totalFiles = plan.categories.reduce((sum, c) => sum + c.files.length, 0);
  const activeFiles = totalFiles - excludedFiles.size;

  return (
    <div className="space-y-1.5">
      <p className="text-xs text-[var(--color-text-secondary)]">
        {plan.categories.length} folders, {activeFiles}/{totalFiles} files in {folderName}
      </p>
      <div className="max-h-[400px] space-y-1 overflow-y-auto">
        {plan.categories.map((cat, i) => {
          const activeInCat = cat.files.filter((f) => !excludedFiles.has(f)).length;
          return (
            <div key={i}>
              <button
                onClick={() => toggle(i)}
                className="flex w-full items-center gap-1.5 rounded px-1 py-1 text-left text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
              >
                {expanded[i] ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                <FolderOpen size={13} strokeWidth={1.5} className="text-[var(--color-accent)]" />
                <span className="font-medium">{cat.folder}/</span>
                <span className="text-xs text-[var(--color-text-secondary)]">
                  {activeInCat}/{cat.files.length} file{cat.files.length !== 1 ? "s" : ""}
                </span>
              </button>
              {expanded[i] && (
                <div className="ml-7 space-y-1 pb-1">
                  {cat.files.map((f, j) => {
                    const excluded = excludedFiles.has(f);
                    return (
                      <label
                        key={j}
                        className={`flex cursor-pointer items-center gap-1.5 px-1 py-0.5 text-xs text-[var(--color-text-secondary)] ${excluded ? "line-through opacity-50" : ""}`}
                      >
                        <input
                          type="checkbox"
                          checked={!excluded}
                          onChange={() => onToggleFile(f)}
                          className="accent-[var(--color-accent)]"
                        />
                        <File size={10} strokeWidth={1.5} />
                        {f.split("/").pop() || f}
                      </label>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export function OrganizeCard({
  folderPath,
  phase,
  plan,
  executeContent,
  error,
  onApprovePlan,
  onCancelPlan,
  onApproveAll,
  onDenyAll,
}: OrganizeCardProps) {
  const folderName = folderPath.split("/").pop() || folderPath;
  const [excludedFiles, setExcludedFiles] = useState<Set<string>>(new Set());

  const toggleFile = (filePath: string) => {
    setExcludedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(filePath)) next.delete(filePath);
      else next.add(filePath);
      return next;
    });
  };

  const handleApprove = () => {
    if (!plan) return;
    const filtered: OrganizePlan = {
      ...plan,
      categories: plan.categories
        .map((cat) => ({
          ...cat,
          files: cat.files.filter((f) => !excludedFiles.has(f)),
        }))
        .filter((cat) => cat.files.length > 0),
    };
    onApprovePlan(JSON.stringify(filtered));
  };

  if (phase === "planning") {
    return null;
  }

  if (phase === "plan-ready" && plan) {
    return (
      <div className="space-y-4">
        <div>
          <h2 className="text-sm font-semibold text-[var(--color-text)]">Organize {folderName}</h2>
        </div>
        <PlanPreview
          plan={plan}
          folderName={folderName}
          excludedFiles={excludedFiles}
          onToggleFile={toggleFile}
        />
        <div className="flex items-center gap-2 border-t border-[var(--color-border)] pt-3">
          <button
            onClick={handleApprove}
            className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3.5 py-1.5 text-sm font-medium text-white hover:opacity-90"
          >
            <Check size={14} /> Approve
          </button>
          <button
            onClick={onCancelPlan}
            className="rounded-md px-3.5 py-1.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
          >
            Cancel
          </button>
        </div>
      </div>
    );
  }

  if (phase === "executing") {
    return null;
  }

  if (phase === "complete") {
    const segments = parseActionBlocks(executeContent);
    const actions = segments.filter((s) => s.type === "action" && s.action).map((s) => s.action!);
    if (actions.length > 0) {
      return (
        <DiffPreview
          actions={actions}
          label={`Organize ${folderName}`}
          onApproveAll={onApproveAll}
          onDenyAll={onDenyAll}
        />
      );
    }
    console.error("[Organize] No action blocks in execute response:", executeContent.slice(0, 500));
    return (
      <div className="flex flex-col items-center gap-2 py-6">
        <AlertCircle size={20} strokeWidth={1.5} className="text-[var(--color-text-secondary)]" />
        <p className="text-sm text-[var(--color-text-secondary)]">
          Could not generate file operations. Try again.
        </p>
      </div>
    );
  }

  if (phase === "error") {
    return (
      <div className="flex flex-col items-center gap-2 py-6">
        <AlertCircle size={20} strokeWidth={1.5} className="text-[var(--color-text-secondary)]" />
        <p className="text-sm text-[var(--color-text-secondary)]">
          {error || "Something went wrong. Try again."}
        </p>
      </div>
    );
  }

  return null;
}
