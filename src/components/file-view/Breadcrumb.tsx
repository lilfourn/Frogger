import { useState, useRef, useEffect } from "react";
import { useFileStore } from "../../stores/fileStore";

export function Breadcrumb() {
  const currentPath = useFileStore((s) => s.currentPath);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) inputRef.current?.select();
  }, [editing]);

  const segments = currentPath.split("/").filter(Boolean);

  function handleSegmentClick(index: number) {
    if (index < 0) {
      navigateTo("/");
    } else {
      navigateTo("/" + segments.slice(0, index + 1).join("/"));
    }
  }

  function startEdit() {
    setEditValue(currentPath);
    setEditing(true);
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter") {
      const trimmed = editValue.trim();
      if (trimmed) navigateTo(trimmed);
      setEditing(false);
    } else if (e.key === "Escape") {
      setEditing(false);
    }
  }

  return (
    <div
      data-testid="breadcrumb"
      className="flex items-center border-b border-[var(--color-border)] px-3 py-1.5"
      onDoubleClick={startEdit}
    >
      {editing ? (
        <input
          ref={inputRef}
          type="text"
          value={editValue}
          onChange={(e) => setEditValue(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={() => setEditing(false)}
          className="w-full rounded border border-[var(--color-accent)] bg-[var(--color-bg)] px-1 text-sm text-[var(--color-text)] outline-none"
        />
      ) : (
        <div className="flex items-center gap-0.5 text-sm">
          <button
            onClick={() => handleSegmentClick(-1)}
            className="rounded px-1 hover:bg-[var(--color-border)]"
          >
            /
          </button>
          {segments.map((seg, i) => (
            <span key={i} className="flex items-center">
              {i > 0 && (
                <span className="text-[var(--color-text-secondary)]">/</span>
              )}
              <button
                onClick={() => handleSegmentClick(i)}
                className={`rounded px-1 hover:bg-[var(--color-border)] ${
                  i === segments.length - 1 ? "font-semibold" : ""
                }`}
              >
                {seg}
              </button>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
