import { invoke } from "@tauri-apps/api/core";
import type { SearchResult } from "../types/search";
import { preflightPermission, retryPermissionAfterFailure } from "./permissionGate";

export async function search(
  query: string,
  limit?: number,
  contextPath?: string,
): Promise<SearchResult[]> {
  const promptTitle = "Search indexed files";
  const allowOnce = await preflightPermission(
    "search",
    contextPath ? [contextPath] : [],
    promptTitle,
  );
  try {
    return await invoke<SearchResult[]>("search", { query, limit, allowOnce });
  } catch (error) {
    if (
      !allowOnce &&
      (await retryPermissionAfterFailure("search", contextPath ? [contextPath] : [], promptTitle))
    ) {
      return invoke<SearchResult[]>("search", { query, limit, allowOnce: true });
    }
    throw error;
  }
}
