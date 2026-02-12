import { invoke } from "@tauri-apps/api/core";

export async function saveApiKey(key: string): Promise<void> {
  return invoke("save_api_key", { key });
}

export async function hasApiKey(): Promise<boolean> {
  return invoke<boolean>("has_api_key");
}

export async function deleteApiKey(): Promise<void> {
  return invoke("delete_api_key");
}

export async function getSetting(key: string): Promise<string | null> {
  return invoke<string | null>("get_setting", { key });
}

export async function setSetting(key: string, value: string): Promise<void> {
  return invoke("set_setting", { key, value });
}

// --- Permission scopes ---

export type PermissionMode = "deny" | "ask" | "allow";

export interface PermissionScope {
  id: number;
  directory_path: string;
  content_scan_mode: PermissionMode;
  modification_mode: PermissionMode;
  ocr_mode: PermissionMode;
  indexing_mode: PermissionMode;
  created_at: string;
}

export interface PermissionCheckRequest {
  action: string;
  paths: string[];
}

export interface PermissionCheckItem {
  path: string;
  capability: string;
  mode: PermissionMode;
  scope_path: string | null;
}

export interface PermissionCheckResponse {
  decision: "allow" | "deny" | "ask";
  blocked: PermissionCheckItem[];
}

export interface PermissionDefaults {
  content_scan_default: PermissionMode;
  modification_default: PermissionMode;
  ocr_default: PermissionMode;
  indexing_default: PermissionMode;
}

export interface PermissionScopeNormalizationReport {
  scanned: number;
  normalized: number;
  merged: number;
  skipped: number;
}

export interface PermissionGrantTargetRequestItem {
  path: string;
  scope_path: string | null;
}

export interface PermissionGrantTarget {
  path: string;
  scope_path: string | null;
  folder_target: string;
  exact_target: string;
  ambiguous: boolean;
}

export async function getPermissionScopes(): Promise<PermissionScope[]> {
  return invoke<PermissionScope[]>("get_permission_scopes");
}

export async function checkPermissionRequest(
  request: PermissionCheckRequest,
): Promise<PermissionCheckResponse> {
  return invoke<PermissionCheckResponse>("check_permission_request", { request });
}

export async function upsertPermissionScope(
  directoryPath: string,
  contentScanMode: PermissionMode,
  modificationMode: PermissionMode,
  ocrMode: PermissionMode,
  indexingMode: PermissionMode,
): Promise<number> {
  return invoke<number>("upsert_permission_scope", {
    directoryPath,
    contentScanMode,
    modificationMode,
    ocrMode,
    indexingMode,
  });
}

export async function deletePermissionScope(id: number): Promise<number> {
  return invoke<number>("delete_permission_scope", { id });
}

export async function getPermissionDefaults(): Promise<PermissionDefaults> {
  return invoke<PermissionDefaults>("get_permission_defaults");
}

export async function setPermissionDefaults(defaults: PermissionDefaults): Promise<void> {
  return invoke("set_permission_defaults", {
    content_scan_default: defaults.content_scan_default,
    modification_default: defaults.modification_default,
    ocr_default: defaults.ocr_default,
    indexing_default: defaults.indexing_default,
  });
}

export async function normalizePermissionScopes(): Promise<PermissionScopeNormalizationReport> {
  return invoke<PermissionScopeNormalizationReport>("normalize_permission_scopes");
}

export async function resolvePermissionGrantTargets(
  items: PermissionGrantTargetRequestItem[],
): Promise<PermissionGrantTarget[]> {
  return invoke<PermissionGrantTarget[]>("resolve_permission_grant_targets", { items });
}

// --- Audit log ---

export interface AuditLogEntry {
  id: number;
  endpoint: string;
  request_summary: string | null;
  tokens_used: number | null;
  cost_usd: number | null;
  created_at: string;
}

export async function getAuditLog(limit?: number): Promise<AuditLogEntry[]> {
  return invoke<AuditLogEntry[]>("get_audit_log", { limit: limit ?? null });
}
