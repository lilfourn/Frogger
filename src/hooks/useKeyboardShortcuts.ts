import { useEffect } from "react";

interface Shortcut {
  key: string;
  meta?: boolean;
  shift?: boolean;
  ctrl?: boolean;
  handler: () => void;
}

export function useKeyboardShortcuts(shortcuts: Shortcut[]) {
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      ) {
        return;
      }

      for (const s of shortcuts) {
        const metaMatch = s.meta ? e.metaKey || e.ctrlKey : true;
        const shiftMatch = s.shift ? e.shiftKey : !e.shiftKey;
        const ctrlMatch = s.ctrl ? e.ctrlKey : true;

        if (e.key.toLowerCase() === s.key.toLowerCase() && metaMatch && shiftMatch && ctrlMatch) {
          e.preventDefault();
          s.handler();
          return;
        }
      }
    }

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [shortcuts]);
}
