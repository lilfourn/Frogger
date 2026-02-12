import { useState, useCallback, useMemo } from "react";
import { ChevronDown, ChevronRight, Check, X, FolderPlus, ArrowRight, Trash2 } from "lucide-react";
import type { FileAction } from "../../utils/actionParser";
import { describeAction, getActionFilePaths } from "../../utils/actionParser";

interface DiffPreviewProps {
  actions: FileAction[];
  label?: string;
  onApproveAll: (actions: FileAction[]) => void;
  onDenyAll: () => void;
}

interface ActionGroup {
  type: string;
  icon: typeof FolderPlus;
  label: string;
  actions: FileAction[];
}

function groupActions(actions: FileAction[]): ActionGroup[] {
  const createDirs = actions.filter((a) => a.tool === "create_directory");
  const moves = actions.filter((a) => a.tool === "move_files");
  const copies = actions.filter((a) => a.tool === "copy_files");
  const deletes = actions.filter((a) => a.tool === "delete_files");
  const renames = actions.filter((a) => a.tool === "rename_file");
  const other = actions.filter(
    (a) =>
      !["create_directory", "move_files", "copy_files", "delete_files", "rename_file"].includes(
        a.tool,
      ),
  );

  const groups: ActionGroup[] = [];
  if (createDirs.length > 0)
    groups.push({
      type: "create",
      icon: FolderPlus,
      label: `Create ${createDirs.length} folder${createDirs.length > 1 ? "s" : ""}`,
      actions: createDirs,
    });
  if (moves.length > 0)
    groups.push({
      type: "move",
      icon: ArrowRight,
      label: `Move ${moves.length} group${moves.length > 1 ? "s" : ""} of files`,
      actions: moves,
    });
  if (copies.length > 0)
    groups.push({
      type: "copy",
      icon: ArrowRight,
      label: `Copy ${copies.length} group${copies.length > 1 ? "s" : ""}`,
      actions: copies,
    });
  if (renames.length > 0)
    groups.push({
      type: "rename",
      icon: ArrowRight,
      label: `Rename ${renames.length} file${renames.length > 1 ? "s" : ""}`,
      actions: renames,
    });
  if (deletes.length > 0)
    groups.push({
      type: "delete",
      icon: Trash2,
      label: `Delete ${deletes.length} item${deletes.length > 1 ? "s" : ""}`,
      actions: deletes,
    });
  if (other.length > 0)
    groups.push({
      type: "other",
      icon: ArrowRight,
      label: `${other.length} other operation${other.length > 1 ? "s" : ""}`,
      actions: other,
    });
  return groups;
}

export function DiffPreview({ actions, label, onApproveAll, onDenyAll }: DiffPreviewProps) {
  const [expanded, setExpanded] = useState(true);
  const [status, setStatus] = useState<"pending" | "running" | "done" | "denied">("pending");
  const useGrouped = actions.length > 5;
  const groups = useMemo(() => (useGrouped ? groupActions(actions) : []), [actions, useGrouped]);

  const handleApprove = useCallback(async () => {
    setStatus("running");
    try {
      await onApproveAll(actions);
      setStatus("done");
    } catch (err) {
      console.error("[Chat] Batch action execution failed:", err);
      setStatus("pending");
    }
  }, [actions, onApproveAll]);

  const handleDeny = useCallback(() => {
    setStatus("denied");
    onDenyAll();
  }, [onDenyAll]);

  const headerText = label
    ? `${label} — ${actions.length} operation${actions.length > 1 ? "s" : ""}`
    : `${actions.length} operation${actions.length > 1 ? "s" : ""}`;

  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs font-medium text-[var(--color-text)]"
      >
        {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        {headerText}
      </button>

      {expanded && (
        <div className="border-t border-[var(--color-border)] px-3 py-2">
          <div className="max-h-[250px] space-y-1.5 overflow-y-auto">
            {useGrouped
              ? groups.map((group) => (
                  <div key={group.type}>
                    <div className="flex items-center gap-1.5 text-xs font-medium text-[var(--color-text)]">
                      <group.icon size={11} strokeWidth={1.5} />
                      {group.label}
                    </div>
                    {group.actions.map((action, j) => (
                      <div key={j} className="ml-4 text-xs text-[var(--color-text-secondary)]">
                        {getActionFilePaths(action)
                          .map((p) => p.split("/").pop())
                          .join(", ")}
                      </div>
                    ))}
                  </div>
                ))
              : actions.map((action, i) => (
                  <div key={i} className="text-xs">
                    <div className="font-medium text-[var(--color-text)]">
                      {describeAction(action)}
                    </div>
                    <div className="text-[var(--color-text-secondary)]">
                      {getActionFilePaths(action)
                        .map((p) => p.split("/").pop())
                        .join(", ")}
                    </div>
                  </div>
                ))}
          </div>
        </div>
      )}

      {status === "pending" && (
        <div className="flex items-center gap-2 border-t border-[var(--color-border)] px-3 py-2">
          <button
            onClick={handleApprove}
            className="flex items-center gap-1 rounded bg-[var(--color-accent)] px-2.5 py-1 text-xs text-white"
          >
            <Check size={12} /> Approve All
          </button>
          <button
            onClick={handleDeny}
            className="flex items-center gap-1 rounded px-2.5 py-1 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]"
          >
            <X size={12} /> Reject
          </button>
        </div>
      )}

      {status === "running" && (
        <div className="border-t border-[var(--color-border)] px-3 py-2 text-xs text-[var(--color-text-secondary)]">
          Executing...
        </div>
      )}

      {status === "done" && (
        <div className="flex items-center gap-1 border-t border-[var(--color-border)] px-3 py-2 text-xs text-[var(--color-accent)]">
          <Check size={12} /> Completed
        </div>
      )}

      {status === "denied" && (
        <div className="flex items-center gap-1 border-t border-[var(--color-border)] px-3 py-2 text-xs text-[var(--color-text-secondary)]">
          <X size={12} /> Rejected
        </div>
      )}
    </div>
  );
}
