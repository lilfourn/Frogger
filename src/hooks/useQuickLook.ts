import { useState, useCallback, useMemo } from "react";
import { detectType } from "../utils/fileType";

export type { PreviewType } from "../utils/fileType";

export function useQuickLook() {
  const [isOpen, setIsOpen] = useState(false);
  const [filePath, setFilePath] = useState<string | null>(null);

  const open = useCallback((path: string) => {
    setFilePath(path);
    setIsOpen(true);
  }, []);

  const close = useCallback(() => {
    setIsOpen(false);
    setFilePath(null);
  }, []);

  const toggle = useCallback((path: string) => {
    setIsOpen((prev) => {
      if (prev) {
        setFilePath(null);
        return false;
      }
      setFilePath(path);
      return true;
    });
  }, []);

  const previewType = useMemo(() => detectType(filePath), [filePath]);

  return { isOpen, filePath, previewType, open, close, toggle };
}
