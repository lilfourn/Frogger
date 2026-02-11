import { invoke } from "@tauri-apps/api/core";
import type { FileEntry } from "../types/file";

export async function listDirectory(path: string): Promise<FileEntry[]> {
  return invoke<FileEntry[]>("list_directory", { path });
}
