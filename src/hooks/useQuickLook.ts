import { useState, useCallback, useMemo } from "react";

export type PreviewType = "image" | "code" | "markdown" | "pdf" | "video" | "unknown";

const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico"]);
const CODE_EXTS = new Set([
  "ts", "tsx", "js", "jsx", "json", "html", "css", "scss",
  "py", "rs", "go", "java", "c", "cpp", "h", "rb", "sh",
  "yaml", "yml", "toml", "xml", "sql", "graphql",
]);
const VIDEO_EXTS = new Set(["mp4", "webm", "ogg", "mov"]);

function getExtension(path: string): string {
  const parts = path.split(".");
  return parts.length > 1 ? parts[parts.length - 1].toLowerCase() : "";
}

function detectType(path: string | null): PreviewType {
  if (!path) return "unknown";
  const ext = getExtension(path);
  if (IMAGE_EXTS.has(ext)) return "image";
  if (ext === "md" || ext === "markdown") return "markdown";
  if (ext === "pdf") return "pdf";
  if (VIDEO_EXTS.has(ext)) return "video";
  if (CODE_EXTS.has(ext)) return "code";
  return "unknown";
}

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
