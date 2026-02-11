import { useEffect, useRef } from "react";

export interface ContextMenuItem {
  label?: string;
  action?: () => void;
  separator?: boolean;
  destructive?: boolean;
  shortcut?: string;
}

interface ContextMenuProps {
  items: ContextMenuItem[];
  position: { x: number; y: number };
  onClose: () => void;
}

export function ContextMenu({ items, position, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose]);

  return (
    <div
      ref={ref}
      className="fixed z-50 min-w-[180px] rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] py-1 shadow-lg"
      style={{ left: position.x, top: position.y }}
    >
      {items.map((item, i) =>
        item.separator ? (
          <div
            key={i}
            data-separator
            className="my-1 border-t border-[var(--color-border)]"
          />
        ) : (
          <button
            key={i}
            onClick={() => {
              item.action?.();
              onClose();
            }}
            className={`flex w-full items-center justify-between px-3 py-1.5 text-left text-sm hover:bg-[var(--color-bg-secondary)] ${
              item.destructive ? "text-red-500" : ""
            }`}
          >
            <span>{item.label}</span>
            {item.shortcut && (
              <span className="ml-4 text-xs text-[var(--color-text-secondary)]">
                {item.shortcut}
              </span>
            )}
          </button>
        ),
      )}
    </div>
  );
}
