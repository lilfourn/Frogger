import { useState, useEffect, useCallback } from "react";
import { Trash2, Plus } from "lucide-react";
import {
  getPermissionScopes,
  getPermissionDefaults,
  setPermissionDefaults,
  upsertPermissionScope,
  deletePermissionScope,
  type PermissionDefaults,
  type PermissionMode,
  type PermissionScope,
} from "../../services/settingsService";

const FULL_ACCESS_DEFAULTS: PermissionDefaults = {
  content_scan_default: "allow",
  modification_default: "allow",
  ocr_default: "allow",
  indexing_default: "allow",
};

const ASK_DEFAULTS: PermissionDefaults = {
  content_scan_default: "ask",
  modification_default: "ask",
  ocr_default: "ask",
  indexing_default: "allow",
};

function isRestrictiveScope(scope: PermissionScope): boolean {
  return (
    scope.content_scan_mode !== "allow" ||
    scope.modification_mode !== "allow" ||
    scope.ocr_mode !== "allow"
  );
}

function profileForDefaults(defaults: PermissionDefaults): "full" | "ask" | "custom" {
  const allAllow =
    defaults.content_scan_default === "allow" &&
    defaults.modification_default === "allow" &&
    defaults.ocr_default === "allow" &&
    defaults.indexing_default === "allow";
  if (allAllow) {
    return "full";
  }

  const allAsk =
    defaults.content_scan_default === "ask" &&
    defaults.modification_default === "ask" &&
    defaults.ocr_default === "ask";
  if (allAsk) {
    return "ask";
  }

  return "custom";
}

export function PermissionSettings() {
  const [scopes, setScopes] = useState<PermissionScope[]>([]);
  const [defaults, setDefaults] = useState<PermissionDefaults>(ASK_DEFAULTS);
  const [newPath, setNewPath] = useState("");
  const [loading, setLoading] = useState(true);
  const [savingProfile, setSavingProfile] = useState(false);

  const load = useCallback(async () => {
    try {
      const [nextScopes, nextDefaults] = await Promise.all([
        getPermissionScopes(),
        getPermissionDefaults(),
      ]);
      setScopes(nextScopes);
      setDefaults(nextDefaults);
    } catch (err) {
      console.error("[PermissionSettings] Failed to load:", err);
      setScopes([]);
      setDefaults(ASK_DEFAULTS);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function handleProfileChange(profile: "full" | "ask") {
    setSavingProfile(true);
    try {
      if (profile === "full") {
        await setPermissionDefaults(FULL_ACCESS_DEFAULTS);
        const latestScopes = await getPermissionScopes();
        const restrictiveScopes = latestScopes.filter(isRestrictiveScope);
        for (const scope of restrictiveScopes) {
          await deletePermissionScope(scope.id);
        }
      } else {
        await setPermissionDefaults(ASK_DEFAULTS);
      }
      await load();
    } finally {
      setSavingProfile(false);
    }
  }

  async function handleAdd() {
    const path = newPath.trim();
    if (!path) {
      return;
    }
    await upsertPermissionScope(path, "ask", "ask", "allow", "allow");
    setNewPath("");
    load();
  }

  async function handleModeChange(
    scope: PermissionScope,
    field: "content_scan_mode" | "modification_mode" | "ocr_mode",
    mode: PermissionMode,
  ) {
    await upsertPermissionScope(
      scope.directory_path,
      field === "content_scan_mode" ? mode : scope.content_scan_mode,
      field === "modification_mode" ? mode : scope.modification_mode,
      field === "ocr_mode" ? mode : scope.ocr_mode,
      "allow",
    );
    load();
  }

  async function handleDelete(id: number) {
    await deletePermissionScope(id);
    load();
  }

  const profile = profileForDefaults(defaults);
  const profileDisabled = loading || savingProfile;

  return (
    <div data-testid="permission-settings" className="space-y-3">
      <div className="rounded border border-[var(--color-border)] px-3 py-2">
        <p className="text-xs font-semibold uppercase text-[var(--color-text-secondary)]">
          Permission Mode
        </p>
        <div className="mt-2 grid grid-cols-2 gap-2">
          <button
            data-testid="permission-profile-full"
            disabled={profileDisabled}
            onClick={() => handleProfileChange("full")}
            className={`rounded border px-2 py-1.5 text-xs ${
              profile === "full"
                ? "border-[var(--color-accent)] bg-[var(--color-accent)] text-white"
                : "border-[var(--color-border)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
            } disabled:opacity-50`}
          >
            Full Access
          </button>
          <button
            data-testid="permission-profile-ask"
            disabled={profileDisabled}
            onClick={() => handleProfileChange("ask")}
            className={`rounded border px-2 py-1.5 text-xs ${
              profile === "ask"
                ? "border-[var(--color-accent)] bg-[var(--color-accent)] text-white"
                : "border-[var(--color-border)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
            } disabled:opacity-50`}
          >
            Ask
          </button>
        </div>
        <p className="mt-2 text-xs text-[var(--color-text-secondary)]">
          Full Access still blocks protected system and program paths.
        </p>
        {profile === "custom" && (
          <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
            Custom defaults are active. Pick Full Access or Ask to normalize.
          </p>
        )}
      </div>

      <div>
        <p className="mb-2 text-xs font-semibold uppercase text-[var(--color-text-secondary)]">
          Advanced Folder Rules
        </p>
        <div className="flex items-center gap-2">
          <input
            data-testid="permission-path-input"
            type="text"
            value={newPath}
            onChange={(e) => setNewPath(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                handleAdd();
              }
            }}
            placeholder="/path/to/directory"
            className="w-full rounded border border-[var(--color-border)] bg-transparent px-2 py-1 text-sm outline-none placeholder:text-[var(--color-text-secondary)]"
          />
          <button
            data-testid="permission-add-btn"
            onClick={handleAdd}
            disabled={!newPath.trim()}
            className="rounded bg-[var(--color-accent)] p-1.5 text-white disabled:opacity-50"
          >
            <Plus size={12} />
          </button>
        </div>
      </div>

      {scopes.length === 0 && (
        <p className="text-xs text-[var(--color-text-secondary)]">
          No custom folder rules. Global mode controls access.
        </p>
      )}

      {scopes.map((scope) => (
        <div key={scope.id} className="rounded border border-[var(--color-border)] px-3 py-2">
          <div className="flex items-center justify-between">
            <span className="truncate text-sm font-medium text-[var(--color-text)]">
              {scope.directory_path}
            </span>
            <button
              onClick={() => handleDelete(scope.id)}
              className="text-[var(--color-text-secondary)] hover:text-red-500"
            >
              <Trash2 size={12} />
            </button>
          </div>
          <div className="mt-2 grid grid-cols-2 gap-2">
            {(
              [
                ["content_scan_mode", "Content Scan"],
                ["modification_mode", "Modification"],
                ["ocr_mode", "OCR"],
              ] as const
            ).map(([field, label]) => (
              <label
                key={field}
                className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]"
              >
                <span className="w-20 shrink-0">{label}</span>
                <select
                  value={scope[field]}
                  onChange={(e) => handleModeChange(scope, field, e.target.value as PermissionMode)}
                  className="w-full rounded border border-[var(--color-border)] bg-transparent px-2 py-1 text-xs text-[var(--color-text)] outline-none"
                >
                  <option value="deny">Deny</option>
                  <option value="ask">Ask</option>
                  <option value="allow">Always Allow</option>
                </select>
              </label>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
