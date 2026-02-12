import type { PreviewType } from "../../hooks/useQuickLook";
import { CodePreview } from "./CodePreview";
import { MarkdownPreview } from "./MarkdownPreview";
import { PdfPreview } from "./PdfPreview";
import { AudioPreview } from "./AudioPreview";

interface QuickLookPanelProps {
  isOpen: boolean;
  filePath: string | null;
  previewType: PreviewType;
  onClose: () => void;
}

function fileName(path: string | null): string {
  if (!path) return "";
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

function PreviewContent({ filePath, previewType }: { filePath: string; previewType: PreviewType }) {
  switch (previewType) {
    case "image":
      return (
        <div className="flex h-full items-center justify-center overflow-auto p-4">
          <img
            src={`asset://localhost/${filePath}`}
            alt={fileName(filePath)}
            className="max-h-full max-w-full object-contain"
          />
        </div>
      );
    case "video":
      return (
        <div className="flex h-full items-center justify-center p-4">
          <video src={`asset://localhost/${filePath}`} controls className="max-h-full max-w-full" />
        </div>
      );
    case "code":
      return <CodePreview filePath={filePath} />;
    case "markdown":
      return <MarkdownPreview filePath={filePath} />;
    case "pdf":
      return <PdfPreview filePath={filePath} />;
    case "audio":
      return <AudioPreview filePath={filePath} />;
    default:
      return (
        <div className="flex h-full items-center justify-center text-sm text-[var(--color-text-secondary)]">
          No preview available
        </div>
      );
  }
}

export function QuickLookPanel({ isOpen, filePath, previewType, onClose }: QuickLookPanelProps) {
  if (!isOpen || !filePath) return null;

  return (
    <div
      data-testid="quick-look-overlay"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
      onClick={onClose}
      tabIndex={-1}
    >
      <div
        className="flex h-[80vh] w-[70vw] flex-col overflow-hidden rounded-lg bg-[var(--color-bg)] shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-[var(--color-border)] px-4 py-2">
          <span className="text-sm font-medium">{fileName(filePath)}</span>
          <button
            onClick={onClose}
            aria-label="Close preview"
            className="rounded px-2 py-0.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
          >
            &times;
          </button>
        </div>
        <div className="flex-1 overflow-auto">
          <PreviewContent filePath={filePath} previewType={previewType} />
        </div>
      </div>
    </div>
  );
}
