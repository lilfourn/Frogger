export type PreviewType = "image" | "code" | "markdown" | "pdf" | "video" | "audio" | "unknown";

const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico"]);
const CODE_EXTS = new Set([
  "ts", "tsx", "js", "jsx", "json", "html", "css", "scss",
  "py", "rs", "go", "java", "c", "cpp", "h", "rb", "sh",
  "yaml", "yml", "toml", "xml", "sql", "graphql",
]);
const VIDEO_EXTS = new Set(["mp4", "webm", "ogg", "mov"]);
const AUDIO_EXTS = new Set(["mp3", "wav", "flac", "aac", "m4a", "ogg", "wma"]);

function getExtension(path: string): string {
  const parts = path.split(".");
  return parts.length > 1 ? parts[parts.length - 1].toLowerCase() : "";
}

export function detectType(path: string | null): PreviewType {
  if (!path) return "unknown";
  const ext = getExtension(path);
  if (IMAGE_EXTS.has(ext)) return "image";
  if (ext === "md" || ext === "markdown") return "markdown";
  if (ext === "pdf") return "pdf";
  if (VIDEO_EXTS.has(ext)) return "video";
  if (AUDIO_EXTS.has(ext)) return "audio";
  if (CODE_EXTS.has(ext)) return "code";
  return "unknown";
}

export function isImageFile(path: string): boolean {
  return IMAGE_EXTS.has(getExtension(path));
}
