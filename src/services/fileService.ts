import { invoke } from "@tauri-apps/api/core";
import type { FileEntry } from "../types/file";
import type { VolumeInfo } from "../types/volume";
import { preflightPermission, retryPermissionAfterFailure } from "./permissionGate";

async function invokeWithPermission<T>(
  command: string,
  args: Record<string, unknown>,
  action: string,
  paths: string[],
  promptTitle: string,
): Promise<T> {
  console.debug(`[fileService] invokeWithPermission: ${command}`, paths);
  const allowOnce = await preflightPermission(action, paths, promptTitle);
  console.debug(`[fileService] preflight done for ${command}, allowOnce=${allowOnce}`);
  try {
    console.debug(`[fileService] invoking Rust command: ${command}`);
    const result = await invoke<T>(command, { ...args, allowOnce });
    console.debug(`[fileService] ${command} returned successfully`);
    return result;
  } catch (error) {
    console.debug(`[fileService] ${command} failed:`, error);
    if (!allowOnce && (await retryPermissionAfterFailure(action, paths, promptTitle))) {
      return invoke<T>(command, { ...args, allowOnce: true });
    }
    throw error;
  }
}

export async function listDirectory(path: string): Promise<FileEntry[]> {
  return invokeWithPermission(
    "list_directory",
    { path },
    "list_directory",
    [path],
    "Access directory contents",
  );
}

export async function getHomeDir(): Promise<string> {
  return invoke<string>("get_home_dir");
}

export async function getMountedVolumes(): Promise<VolumeInfo[]> {
  return invoke<VolumeInfo[]>("get_mounted_volumes");
}

export async function createDirectory(path: string): Promise<void> {
  return invokeWithPermission(
    "create_directory",
    { path },
    "create_directory",
    [path],
    "Create directory",
  );
}

export async function renameFile(source: string, destination: string): Promise<void> {
  return invokeWithPermission(
    "rename_file",
    { source, destination },
    "rename_file",
    [source, destination],
    "Rename file",
  );
}

export async function moveFiles(sources: string[], destDir: string): Promise<string[]> {
  return invokeWithPermission(
    "move_files",
    { sources, destDir },
    "move_files",
    [...sources, destDir],
    "Move files",
  );
}

export async function copyFiles(sources: string[], destDir: string): Promise<string[]> {
  return invokeWithPermission(
    "copy_files",
    { sources, destDir },
    "copy_files",
    [...sources, destDir],
    "Copy files",
  );
}

export async function deleteFiles(paths: string[]): Promise<void> {
  return invokeWithPermission("delete_files", { paths }, "delete_files", paths, "Delete files");
}

export async function copyFilesWithProgress(
  sources: string[],
  destDir: string,
  operationId?: string,
): Promise<string[]> {
  return invokeWithPermission(
    "copy_files_with_progress",
    { sources, destDir, operationId },
    "copy_files_with_progress",
    [...sources, destDir],
    "Copy files",
  );
}

export async function cancelOperation(operationId?: string): Promise<void> {
  return invoke("cancel_operation", { operationId });
}

export async function undoOperation(): Promise<string> {
  return invokeWithPermission(
    "undo_operation",
    {},
    "undo_operation",
    [],
    "Undo previous operation",
  );
}

export async function redoOperation(): Promise<string> {
  return invokeWithPermission(
    "redo_operation",
    {},
    "redo_operation",
    [],
    "Redo previous operation",
  );
}

export async function readFileText(path: string): Promise<string> {
  return invokeWithPermission(
    "read_file_text",
    { path },
    "read_file_text",
    [path],
    "Read file contents",
  );
}

export async function findLargeFiles(directory: string, minSize: number): Promise<FileEntry[]> {
  return invokeWithPermission(
    "find_large_files",
    { directory, minSize },
    "find_large_files",
    [directory],
    "Scan for large files",
  );
}

export async function findOldFiles(directory: string, olderThanDays: number): Promise<FileEntry[]> {
  return invokeWithPermission(
    "find_old_files",
    { directory, olderThanDays },
    "find_old_files",
    [directory],
    "Scan for old files",
  );
}

export async function findDuplicates(directory: string): Promise<FileEntry[][]> {
  return invokeWithPermission(
    "find_duplicates",
    { directory },
    "find_duplicates",
    [directory],
    "Scan for duplicate files",
  );
}

export interface ProjectInfo {
  project_type: string;
  marker_file: string;
  directory: string;
}

export async function openFile(path: string): Promise<void> {
  return invokeWithPermission("open_file", { path }, "open_file", [path], "Open file");
}

export async function detectProjectType(directory: string): Promise<ProjectInfo | null> {
  return invokeWithPermission(
    "detect_project_type",
    { directory },
    "detect_project_type",
    [directory],
    "Detect project type",
  );
}
