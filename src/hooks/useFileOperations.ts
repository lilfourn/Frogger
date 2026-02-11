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
        await createDirectory(`${currentPath}/${name}`);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [currentPath, refresh, setError],
  );

  const handleRename = useCallback(
    async (source: string, newName: string) => {
      const parent = source.replace(/\/[^/]+\/?$/, "");
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
