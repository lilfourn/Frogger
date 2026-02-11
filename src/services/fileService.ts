import { invoke } from "@tauri-apps/api/core";
import type { FileEntry } from "../types/file";
import type { VolumeInfo } from "../types/volume";

export async function listDirectory(path: string): Promise<FileEntry[]> {
  return invoke<FileEntry[]>("list_directory", { path });
}

export async function getHomeDir(): Promise<string> {
  return invoke<string>("get_home_dir");
}

export async function getMountedVolumes(): Promise<VolumeInfo[]> {
  return invoke<VolumeInfo[]>("get_mounted_volumes");
}
