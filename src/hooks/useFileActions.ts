import { useCallback } from "react";
import type { FileAction } from "../utils/actionParser";
import { validateActionArgs } from "../utils/actionParser";
import {
  moveFiles,
  copyFiles,
  renameFile,
  deleteFiles,
  createDirectory,
} from "../services/fileService";

export function useFileActions() {
  const execute = useCallback(async (action: FileAction) => {
    const { tool, args } = action;
    if (!validateActionArgs(tool, args ?? {})) {
      throw new Error(`Invalid arguments for ${tool}`);
    }
    switch (tool) {
      case "move_files":
        await moveFiles(args.sources as string[], args.dest_dir as string);
        break;
      case "copy_files":
        await copyFiles(args.sources as string[], args.dest_dir as string);
        break;
      case "rename_file":
        await renameFile(args.source as string, args.destination as string);
        break;
      case "delete_files":
        await deleteFiles(args.paths as string[]);
        break;
      case "create_directory":
        await createDirectory(args.path as string);
        break;
    }
  }, []);

  return { execute };
}
