import { invoke } from "@tauri-apps/api/core";
import type { SearchResult } from "../types/search";

export async function search(query: string, limit?: number): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search", { query, limit });
}
