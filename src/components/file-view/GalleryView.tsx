import { useState, useCallback, useRef, useEffect } from "react";
import type { FileEntry } from "../../types/file";
import { FileIcon } from "../shared/FileIcon";
import { isImageFile, detectType } from "../../utils/fileType";

interface GalleryViewProps {
  entries: FileEntry[];
  onSelect: (entry: FileEntry) => void;
  onOpen: (entry: FileEntry) => void;
  onItemContextMenu: (e: React.MouseEvent, entry: FileEntry) => void;
  selectedPaths: Set<string>;
  focusIndex?: number;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getCarouselStyle(distance: number) {
  if (distance === 0) return { transform: "scale(1.15)", opacity: 1 };
  if (distance === 1) return { transform: "scale(0.85)", opacity: 0.8 };
  if (distance === 2) return { transform: "scale(0.7)", opacity: 0.5 };
  return { transform: "scale(0.6)", opacity: 0.3 };
}

export function GalleryView({
  entries,
  onSelect,
  onOpen,
  onItemContextMenu,
}: GalleryViewProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);

  const current = entries[selectedIndex];

  useEffect(() => {
    containerRef.current?.focus();
  }, []);

  const selectAt = useCallback(
    (index: number) => {
      setSelectedIndex(index);
      if (entries[index]) onSelect(entries[index]);
    },
    [entries, onSelect],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case "ArrowRight": {
          e.preventDefault();
          e.stopPropagation();
          if (selectedIndex < entries.length - 1) selectAt(selectedIndex + 1);
          break;
        }
        case "ArrowLeft": {
          e.preventDefault();
          e.stopPropagation();
          if (selectedIndex > 0) selectAt(selectedIndex - 1);
          break;
        }
        case "Enter": {
          e.stopPropagation();
          if (current) onOpen(current);
          break;
        }
      }
    },
    [selectedIndex, entries.length, selectAt, current, onOpen],
  );

  return (
    <div
      data-testid="gallery-view"
      ref={containerRef}
      className="flex h-full flex-col outline-none"
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      <div
        data-testid="gallery-preview"
        className="flex flex-1 flex-col items-center justify-center overflow-hidden p-4"
      >
        {current && <Preview entry={current} />}
      </div>

      <div
        data-testid="gallery-filmstrip"
        className="flex h-[120px] shrink-0 items-center justify-center overflow-x-hidden border-t border-[var(--color-border)] p-2"
      >
        {entries.map((entry, idx) => {
          const distance = Math.abs(idx - selectedIndex);
          const style = getCarouselStyle(distance);

          return (
            <div
              key={entry.path}
              data-testid="filmstrip-item"
              className={`mx-1 flex w-[80px] shrink-0 cursor-pointer flex-col items-center transition-all duration-200 ${
                idx === selectedIndex ? "ring-2 ring-[var(--color-accent)] rounded" : ""
              }`}
              style={style}
              onClick={() => selectAt(idx)}
              onDoubleClick={() => onOpen(entry)}
              onContextMenu={(e) => onItemContextMenu(e, entry)}
            >
              <div className="flex h-[80px] w-[80px] items-center justify-center overflow-hidden rounded">
                {isImageFile(entry.path) ? (
                  <img
                    src={`asset://localhost/${entry.path}`}
                    alt={entry.name}
                    className="h-full w-full object-cover"
                    loading="lazy"
                  />
                ) : (
                  <FileIcon isDirectory={entry.is_directory} size={32} />
                )}
              </div>
              <span className="w-full truncate text-center text-[10px]">{entry.name}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function Preview({ entry }: { entry: FileEntry }) {
  const type = detectType(entry.path);

  if (type === "image") {
    return (
      <>
        <img
          src={`asset://localhost/${entry.path}`}
          alt={entry.name}
          className="max-h-full max-w-full object-contain"
        />
        <span className="mt-2 text-sm text-[var(--color-text-secondary)]">{entry.name}</span>
      </>
    );
  }

  if (type === "video") {
    return (
      <>
        <video
          src={`asset://localhost/${entry.path}`}
          controls
          className="max-h-full max-w-full"
        />
        <span className="mt-2 text-sm text-[var(--color-text-secondary)]">{entry.name}</span>
      </>
    );
  }

  return (
    <div className="flex flex-col items-center gap-3">
      <FileIcon isDirectory={entry.is_directory} size={120} />
      <span className="text-sm font-medium">{entry.name}</span>
      {entry.size_bytes != null && (
        <span className="text-xs text-[var(--color-text-secondary)]">
          {formatSize(entry.size_bytes)}
        </span>
      )}
    </div>
  );
}
