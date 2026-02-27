import {
  FolderOpen,
  Check,
  AlertCircle,
  ChevronDown,
  ChevronRight,
  File,
  Pencil,
  RotateCcw,
} from "lucide-react";
import { useState, useMemo, useRef, useEffect, useCallback } from "react";
import { parseActionBlocks } from "../../utils/actionParser";
import { DiffPreview } from "./DiffPreview";
import {
  createEditablePlan,
  toggleFile,
  editFileName,
  editFolderLabel,
  setRenameEnabled,
  countActiveFiles,
  countTotalFiles,
  countRenames,
  hasAnyRenames,
  toApprovedPlanJson,
  buildFolderTree,
} from "../../utils/editablePlan";
import type { FileAction, OrganizePlan } from "../../utils/actionParser";
import type { EditablePlan, EditableFile, FolderTreeNode } from "../../utils/editablePlan";
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
  onOpenPath?: (path: string) => Promise<void>;
  onRetry?: () => void;
}

interface PlanTreeViewProps {
  editablePlan: EditablePlan;
  folderName: string;
  expanded: Set<string>;
  onToggleExpand: (path: string) => void;
  onToggleFile: (path: string) => void;
  onEditName: (path: string, newName: string) => void;
  onEditFolder: (folderId: string, newLabel: string) => void;
}

const MISSING_SOURCE_ERROR_RE =
  /Missing source path for [^:]+ action \d+\/\d+: (.+?)\. Re-run organization plan and try again\./;

function extractMissingSourcePath(error: string): string | null {
  const path = error.match(MISSING_SOURCE_ERROR_RE)?.[1]?.trim();
  return path && path.length > 0 ? path : null;
}

function parentPathForOpen(path: string): string {
  const normalized = path.trim().replace(/[\\/]+$/, "");
  const idx = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  if (idx > 0) {
    return normalized.slice(0, idx);
  }
  if (idx === 0) {
    return normalized.slice(0, 1);
  }
  return normalized;
}

function InlineEdit({
  value,
  onSave,
  className,
}: {
  value: string;
  onSave: (newValue: string) => void;
  className?: string;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [editing]);

  const commit = useCallback(() => {
    const trimmed = draft.trim();
    if (trimmed && trimmed !== value) {
      onSave(trimmed);
    } else {
      setDraft(value);
    }
    setEditing(false);
  }, [draft, value, onSave]);

  if (editing) {
    return (
      <input
        ref={inputRef}
        value={draft}
        onClick={(e) => e.stopPropagation()}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") {
            setDraft(value);
            setEditing(false);
          }
        }}
        className={`rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-1 py-0 text-xs outline-none focus:border-[var(--color-accent)] ${className ?? ""}`}
      />
    );
  }

  return (
    <span
      role="button"
      tabIndex={0}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setDraft(value);
        setEditing(true);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          setDraft(value);
          setEditing(true);
        }
      }}
      className="inline-flex cursor-pointer items-center gap-0.5 text-[var(--color-text-secondary)] opacity-0 transition-opacity group-hover:opacity-100 hover:text-[var(--color-accent)]"
      title="Edit name"
    >
      <Pencil size={10} strokeWidth={1.5} />
    </span>
  );
}

function PlanSummary({
  editablePlan,
  folderName,
  roots,
}: {
  editablePlan: EditablePlan;
  folderName: string;
  roots: FolderTreeNode[];
}) {
  const total = countTotalFiles(editablePlan);
  const active = countActiveFiles(editablePlan);
  const renames = countRenames(editablePlan);

  return (
    <div className="space-y-1">
      <p className="text-xs text-[var(--color-text-secondary)]">
        {editablePlan.folders.length} folders · {active}/{total} files
        {renames > 0 ? ` · ${renames} renamed` : ""} in {folderName}
      </p>
      {roots.length >= 2 && (
        <div className="flex flex-wrap gap-x-3 text-xs text-[var(--color-text-secondary)]">
          {roots.map((r) => (
            <span key={r.path}>
              {r.name} <span className="opacity-60">{r.fileCount}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function FileRow({
  file,
  renameEnabled,
  onToggle,
  onEditName,
}: {
  file: EditableFile;
  renameEnabled: boolean;
  onToggle: () => void;
  onEditName: (name: string) => void;
}) {
  const displayName = file.editedName ?? file.suggestedName;
  const hasRename = renameEnabled && !!displayName;

  return (
    <label
      className={`group flex cursor-pointer items-center gap-1.5 px-1 py-0.5 text-xs text-[var(--color-text-secondary)] ${!file.included ? "line-through opacity-50" : ""}`}
    >
      <input
        type="checkbox"
        checked={file.included}
        onChange={onToggle}
        className="accent-[var(--color-accent)]"
      />
      <File size={10} strokeWidth={1.5} />
      {hasRename ? (
        <span className="inline-flex items-center gap-0.5">
          <span className="line-through opacity-50">{file.originalName}</span>
          <span className="mx-0.5 text-[var(--color-text-secondary)]">→</span>
          <span className="text-[var(--color-accent)]">{displayName}</span>
          <span className="rounded-full bg-[var(--color-accent)]/15 px-1.5 text-[10px] text-[var(--color-accent)]">
            renamed
          </span>
        </span>
      ) : (
        file.originalName
      )}
      {(hasRename || file.suggestedName) && renameEnabled && (
        <InlineEdit
          value={displayName || file.originalName}
          onSave={onEditName}
        />
      )}
    </label>
  );
}

function TreeNode({
  node,
  depth,
  expanded,
  onToggleExpand,
  editablePlan,
  onToggleFile,
  onEditName,
  onEditFolder,
}: {
  node: FolderTreeNode;
  depth: number;
  expanded: Set<string>;
  onToggleExpand: (path: string) => void;
  editablePlan: EditablePlan;
  onToggleFile: (path: string) => void;
  onEditName: (path: string, name: string) => void;
  onEditFolder: (folderId: string, label: string) => void;
}) {
  // Single-child collapsing: combine names when node has 1 child and no own files
  if (node.children.length === 1 && !node.folder) {
    const child = node.children[0];
    const collapsed: FolderTreeNode = {
      ...child,
      name: `${node.name}/${child.name}`,
      path: child.path,
    };
    return (
      <TreeNode
        node={collapsed}
        depth={depth}
        expanded={expanded}
        onToggleExpand={onToggleExpand}
        editablePlan={editablePlan}
        onToggleFile={onToggleFile}
        onEditName={onEditName}
        onEditFolder={onEditFolder}
      />
    );
  }

  const isExpanded = expanded.has(node.path);
  const hasChildren = node.children.length > 0 || node.folder !== null;

  return (
    <div>
      <button
        onClick={() => onToggleExpand(node.path)}
        className="group flex w-full items-center gap-1.5 rounded px-1 py-1 text-left text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
        style={{ paddingLeft: `${depth * 16 + 4}px` }}
      >
        {hasChildren ? (
          isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />
        ) : (
          <span className="w-3" />
        )}
        <FolderOpen size={13} strokeWidth={1.5} className="text-[var(--color-accent)]" />
        <span className="font-medium">{node.name}</span>
        {node.folder && (
          <InlineEdit
            value={node.folder.label.replace(/\/$/, "")}
            onSave={(val) => onEditFolder(node.folder!.id, val + "/")}
          />
        )}
        <span className="text-xs text-[var(--color-text-secondary)]">
          {node.activeFileCount}/{node.fileCount} file{node.fileCount !== 1 ? "s" : ""}
        </span>
        {!isExpanded && node.renameCount > 0 && (
          <span className="rounded-full bg-[var(--color-accent)]/15 px-1.5 text-[10px] text-[var(--color-accent)]">
            {node.renameCount} renamed
          </span>
        )}
      </button>
      {isExpanded && (
        <>
          {node.folder?.files.map((f, j) => (
            <div key={j} style={{ paddingLeft: `${(depth + 1) * 16 + 4}px` }}>
              <FileRow
                file={f}
                renameEnabled={editablePlan.renameEnabled}
                onToggle={() => onToggleFile(f.path)}
                onEditName={(name) => onEditName(f.path, name)}
              />
            </div>
          ))}
          {node.children.map((child) => (
            <TreeNode
              key={child.path}
              node={child}
              depth={depth + 1}
              expanded={expanded}
              onToggleExpand={onToggleExpand}
              editablePlan={editablePlan}
              onToggleFile={onToggleFile}
              onEditName={onEditName}
              onEditFolder={onEditFolder}
            />
          ))}
        </>
      )}
    </div>
  );
}

function PlanTreeView({
  editablePlan,
  folderName,
  expanded,
  onToggleExpand,
  onToggleFile,
  onEditName,
  onEditFolder,
}: PlanTreeViewProps) {
  const roots = useMemo(() => buildFolderTree(editablePlan), [editablePlan]);

  return (
    <div className="space-y-1.5">
      <PlanSummary editablePlan={editablePlan} folderName={folderName} roots={roots} />
      <div className="max-h-[400px] overflow-y-auto">
        {roots.map((node) => (
          <TreeNode
            key={node.path}
            node={node}
            depth={0}
            expanded={expanded}
            onToggleExpand={onToggleExpand}
            editablePlan={editablePlan}
            onToggleFile={onToggleFile}
            onEditName={onEditName}
            onEditFolder={onEditFolder}
          />
        ))}
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
  onOpenPath,
  onRetry,
}: OrganizeCardProps) {
  const folderName = folderPath.split("/").pop() || folderPath;
  const [editablePlan, setEditablePlan] = useState<EditablePlan | null>(() =>
    plan ? createEditablePlan(plan) : null,
  );
  const [prevPlan, setPrevPlan] = useState(plan);
  if (plan !== prevPlan) {
    setPrevPlan(plan);
    setEditablePlan(plan ? createEditablePlan(plan) : null);
  }
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [openLocationError, setOpenLocationError] = useState("");

  const toggleExpand = useCallback((path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  const missingSourcePath = extractMissingSourcePath(error);
  const openPath = missingSourcePath ? parentPathForOpen(missingSourcePath) : "";

  const handleToggleFile = (path: string) =>
    setEditablePlan((prev) => (prev ? toggleFile(prev, path) : prev));

  const handleEditName = (path: string, name: string) =>
    setEditablePlan((prev) => (prev ? editFileName(prev, path, name) : prev));

  const handleEditFolder = (id: string, label: string) =>
    setEditablePlan((prev) => (prev ? editFolderLabel(prev, id, label) : prev));

  const handleToggleRename = (enabled: boolean) =>
    setEditablePlan((prev) => (prev ? setRenameEnabled(prev, enabled) : prev));

  const handleApprove = () => {
    if (!editablePlan || !plan) return;
    onApprovePlan(toApprovedPlanJson(editablePlan, plan));
  };

  if (phase === "planning") {
    return null;
  }

  if (phase === "plan-ready" && plan && editablePlan) {
    const otherRatio = plan.stats?.other_ratio;
    const showOtherWarning = typeof otherRatio === "number" && otherRatio > 0.1;
    const hasPackingStats =
      typeof plan.stats?.max_children_observed === "number" ||
      typeof plan.stats?.folders_over_target === "number" ||
      typeof plan.stats?.packing_llm_calls === "number";
    const showRenameToggle = hasAnyRenames(editablePlan);
    return (
      <div className="space-y-4">
        <div>
          <h2 className="text-sm font-semibold text-[var(--color-text)]">Organize {folderName}</h2>
          {showOtherWarning && (
            <p className="mt-1 text-xs text-amber-600">
              High unclassified ratio: {(otherRatio * 100).toFixed(1)}%
            </p>
          )}
          {hasPackingStats && (
            <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
              Packing quality: max children {plan.stats?.max_children_observed ?? 0}, over target{" "}
              {plan.stats?.folders_over_target ?? plan.stats?.capacity_overflow_dirs ?? 0}, LLM
              refinements {plan.stats?.packing_llm_calls ?? 0}
            </p>
          )}
        </div>
        {showRenameToggle && (
          <label className="flex cursor-pointer items-center gap-2 text-xs text-[var(--color-text-secondary)]">
            <input
              type="checkbox"
              checked={editablePlan.renameEnabled}
              onChange={(e) => handleToggleRename(e.target.checked)}
              className="accent-[var(--color-accent)]"
            />
            Rename files with unclear names
          </label>
        )}
        <PlanTreeView
          editablePlan={editablePlan}
          folderName={folderName}
          expanded={expanded}
          onToggleExpand={toggleExpand}
          onToggleFile={handleToggleFile}
          onEditName={handleEditName}
          onEditFolder={handleEditFolder}
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
          denyLabel="Cancel"
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
        <button
          onClick={onDenyAll}
          className="rounded-md px-3.5 py-1.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
        >
          Cancel
        </button>
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
        {missingSourcePath && onOpenPath && (
          <>
            <p className="max-w-full break-all rounded bg-[var(--color-bg-secondary)] px-2 py-1 font-mono text-xs text-[var(--color-text-secondary)]">
              {missingSourcePath}
            </p>
            <button
              onClick={() => {
                setOpenLocationError("");
                void onOpenPath(openPath).catch((err: unknown) => {
                  const message =
                    typeof err === "string"
                      ? err
                      : "Could not open file location from this device.";
                  setOpenLocationError(message);
                });
              }}
              className="rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
            >
              Open file location
            </button>
            {openLocationError && (
              <p className="max-w-full break-all text-xs text-[var(--color-text-secondary)]">
                {openLocationError}
              </p>
            )}
          </>
        )}
        <div className="flex items-center gap-2">
          {onRetry && (
            <button
              onClick={onRetry}
              className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3.5 py-1.5 text-sm font-medium text-white hover:opacity-90"
            >
              <RotateCcw size={14} /> Retry
            </button>
          )}
          <button
            onClick={onDenyAll}
            className="rounded-md px-3.5 py-1.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
          >
            Cancel
          </button>
        </div>
      </div>
    );
  }

  return null;
}
