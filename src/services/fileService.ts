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

export async function createDirectory(path: string): Promise<void> {
  return invoke("create_directory", { path });
}

export async function renameFile(source: string, destination: string): Promise<void> {
  return invoke("rename_file", { source, destination });
}

export async function moveFiles(sources: string[], destDir: string): Promise<string[]> {
  return invoke("move_files", { sources, destDir });
}

export async function copyFiles(sources: string[], destDir: string): Promise<string[]> {
  return invoke("copy_files", { sources, destDir });
}

export async function deleteFiles(paths: string[]): Promise<void> {
  return invoke("delete_files", { paths });
}

export async function undoOperation(): Promise<string> {
  return invoke("undo_operation");
}

export async function redoOperation(): Promise<string> {
  return invoke("redo_operation");
}
