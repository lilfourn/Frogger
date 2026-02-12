import { requestPermissionPrompt } from "../stores/permissionPromptStore";
import {
  checkPermissionRequest,
  getPermissionDefaults,
  getPermissionScopes,
  normalizePermissionScopes,
  resolvePermissionGrantTargets,
  upsertPermissionScope,
  type PermissionCheckItem,
  type PermissionGrantTarget,
  type PermissionMode,
} from "./settingsService";
import { normalizePath, normalizePaths } from "../utils/paths";

type PermissionModeField = "content_scan_mode" | "modification_mode" | "ocr_mode" | "indexing_mode";

type AlwaysAllowScope = "folder" | "exact";

interface ScopeModes {
  content_scan_mode: PermissionMode;
  modification_mode: PermissionMode;
  ocr_mode: PermissionMode;
  indexing_mode: PermissionMode;
}

function blockedKey(item: Pick<PermissionCheckItem, "path" | "scope_path">): string {
  return `${normalizePath(item.path)}|${item.scope_path ? normalizePath(item.scope_path) : ""}`;
}

function formatBlockedList(blocked: PermissionCheckItem[]): string {
  if (blocked.length === 0) {
    return "";
  }
  const sample = blocked.slice(0, 4).map((item) => {
    const scope = item.scope_path ? ` (scope: ${item.scope_path})` : "";
    return `- ${item.capability}: ${item.path}${scope}`;
  });
  const suffix = blocked.length > 4 ? `\n- ...and ${blocked.length - 4} more` : "";
  return `${sample.join("\n")}${suffix}`;
}

function capabilityToField(capability: string): PermissionModeField | null {
  switch (capability) {
    case "content_scan":
      return "content_scan_mode";
    case "modification":
      return "modification_mode";
    case "ocr":
      return "ocr_mode";
    case "indexing":
      return "indexing_mode";
    default:
      return null;
  }
}

function modesFromScope(scope: {
  content_scan_mode: PermissionMode;
  modification_mode: PermissionMode;
  ocr_mode: PermissionMode;
  indexing_mode: PermissionMode;
}): ScopeModes {
  return {
    content_scan_mode: scope.content_scan_mode,
    modification_mode: scope.modification_mode,
    ocr_mode: scope.ocr_mode,
    indexing_mode: scope.indexing_mode,
  };
}

async function resolveGrantTargets(
  blocked: PermissionCheckItem[],
): Promise<PermissionGrantTarget[]> {
  const uniqueItems = Array.from(
    new Map(blocked.map((item) => [blockedKey(item), item])).values(),
  ).map((item) => ({
    path: item.path,
    scope_path: item.scope_path,
  }));

  if (uniqueItems.length === 0) {
    return [];
  }

  try {
    return await resolvePermissionGrantTargets(uniqueItems);
  } catch (err) {
    console.error("[Permission] Failed to resolve grant targets:", err);
    return [];
  }
}

async function persistAlwaysAllow(
  blocked: PermissionCheckItem[],
  scopeChoice: AlwaysAllowScope,
  grantTargets: PermissionGrantTarget[],
): Promise<void> {
  const persistable = blocked.filter((item) => capabilityToField(item.capability) !== null);
  if (persistable.length === 0) {
    return;
  }

  const [scopes, defaults] = await Promise.all([getPermissionScopes(), getPermissionDefaults()]);
  const defaultScopeModes: ScopeModes = {
    content_scan_mode: defaults.content_scan_default,
    modification_mode: defaults.modification_default,
    ocr_mode: defaults.ocr_default,
    indexing_mode: defaults.indexing_default,
  };

  const grantTargetByKey = new Map(grantTargets.map((target) => [blockedKey(target), target]));
  const existingScopeByPath = new Map(
    scopes.map((scope) => [normalizePath(scope.directory_path), scope]),
  );
  const updates = new Map<string, ScopeModes>();

  for (const item of persistable) {
    const field = capabilityToField(item.capability);
    if (!field) {
      continue;
    }

    const target = grantTargetByKey.get(blockedKey(item));
    const fallbackTarget = item.scope_path
      ? normalizePath(item.scope_path)
      : normalizePath(item.path);
    const chosenTarget =
      scopeChoice === "exact"
        ? (target?.exact_target ?? fallbackTarget)
        : (target?.folder_target ?? fallbackTarget);
    const targetPath = normalizePath(chosenTarget);
    if (!targetPath) {
      continue;
    }

    const existingScope = existingScopeByPath.get(targetPath);
    const baseModes =
      updates.get(targetPath) ??
      (existingScope ? modesFromScope(existingScope) : { ...defaultScopeModes });

    baseModes[field] = "allow";
    updates.set(targetPath, baseModes);
  }

  for (const [directoryPath, modes] of updates) {
    await upsertPermissionScope(
      directoryPath,
      modes.content_scan_mode,
      modes.modification_mode,
      modes.ocr_mode,
      modes.indexing_mode,
    );
  }

  if (updates.size > 0) {
    await normalizePermissionScopes();
  }
}

export async function preflightPermission(
  action: string,
  paths: string[],
  promptTitle: string,
): Promise<boolean> {
  const normalized = normalizePaths(paths);
  if (normalized.length === 0) {
    return false;
  }

  console.debug(`[Permission] preflight: action=${action}, paths=`, normalized);
  const result = await checkPermissionRequest({ action, paths: normalized });
  console.debug(`[Permission] preflight result: decision=${result.decision}, blocked=${result.blocked?.length ?? 0}`);
  const blocked = Array.isArray(result.blocked) ? result.blocked : [];
  if (result.decision === "allow") {
    return false;
  }

  if (result.decision === "deny") {
    const details = formatBlockedList(blocked);
    throw new Error(`Permission denied.\n${details}`);
  }

  console.debug("[Permission] decision=ask, showing permission prompt modal...");
  const grantTargets = await resolveGrantTargets(blocked);
  const allowExactPath = grantTargets.some((target) => target.ambiguous);
  const decision = await requestPermissionPrompt({
    title: promptTitle,
    action,
    promptKind: "initial",
    blocked,
    allowAlways: blocked.length > 0,
    allowExactPath,
  });
  console.debug(`[Permission] user decision: ${decision}`);

  if (decision === "deny") {
    throw new Error("Permission denied by user");
  }

  if (decision === "always_allow_folder" || decision === "always_allow_exact") {
    try {
      await persistAlwaysAllow(
        blocked,
        decision === "always_allow_exact" ? "exact" : "folder",
        grantTargets,
      );
    } catch (error) {
      console.error("[Permission] failed to persist always-allow scopes", error);
    }
  }

  return true;
}

export async function confirmAllowOnceFallback(
  promptTitle: string,
  action: string,
  blocked: PermissionCheckItem[] = [],
): Promise<boolean> {
  const decision = await requestPermissionPrompt({
    title: promptTitle,
    action,
    promptKind: "retry",
    blocked,
    allowAlways: false,
    allowExactPath: false,
  });
  return decision === "allow_once";
}

export async function retryPermissionAfterFailure(
  action: string,
  paths: string[],
  promptTitle: string,
): Promise<boolean> {
  const normalized = normalizePaths(paths);
  if (normalized.length === 0) {
    return false;
  }

  try {
    const result = await checkPermissionRequest({ action, paths: normalized });
    if (result.decision !== "ask") {
      return false;
    }
    const blocked = Array.isArray(result.blocked) ? result.blocked : [];
    return confirmAllowOnceFallback(promptTitle, action, blocked);
  } catch (err) {
    console.error("[Permission] Retry check failed:", err);
    return false;
  }
}
