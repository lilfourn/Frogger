import { useCallback } from "react";
import { useFileStore } from "../stores/fileStore";
import {
  createDirectory,
  renameFile,
  deleteFiles,
  undoOperation,
  redoOperation,
  listDirectory,
} from "../services/fileService";
import { normalizePath } from "../utils/paths";

export function useFileOperations() {
  const currentPath = useFileStore((s) => s.currentPath);
  const setEntries = useFileStore((s) => s.setEntries);
  const setError = useFileStore((s) => s.setError);

  const refresh = useCallback(async () => {
    if (!currentPath) return;
    try {
      const entries = await listDirectory(currentPath);
      setEntries(entries);
    } catch (e) {
      setError(String(e));
    }
  }, [currentPath, setEntries, setError]);

  const handleCreateDir = useCallback(
    async (name: string) => {
      try {
        const basePath = normalizePath(currentPath);
        await createDirectory(`${basePath}/${name}`);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [currentPath, refresh, setError],
  );

  const handleRename = useCallback(
    async (source: string, newName: string) => {
      const normalizedSource = normalizePath(source);
      const segments = normalizedSource.split("/");
      segments.pop();
      const parent = segments.join("/") || "/";
      const destination = `${parent}/${newName}`;
      try {
        await renameFile(source, destination);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh, setError],
  );

  const handleDelete = useCallback(
    async (paths: string[]) => {
      try {
        await deleteFiles(paths);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh, setError],
  );

  const handleUndo = useCallback(async () => {
    try {
      await undoOperation();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [refresh, setError]);

  const handleRedo = useCallback(async () => {
    try {
      await redoOperation();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [refresh, setError]);

  return {
    refresh,
    createDir: handleCreateDir,
    rename: handleRename,
    deleteFiles: handleDelete,
    undo: handleUndo,
    redo: handleRedo,
  };
}
