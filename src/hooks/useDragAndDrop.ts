import { useState, useCallback } from "react";

const MIME = "application/x-frogger-paths";

interface UseDragAndDropOptions {
  onDrop: (paths: string[]) => void;
}

export function useDragAndDrop({ onDrop }: UseDragAndDropOptions) {
  const [isDragging, setIsDragging] = useState(false);
  const [isOver, setIsOver] = useState(false);

  const dragHandlers = {
    onDragStart: useCallback((e: React.DragEvent) => {
      setIsDragging(true);
      const paths = e.dataTransfer.getData(MIME);
      if (!paths) {
        e.dataTransfer.setData(MIME, "[]");
      }
    }, []),
    onDragEnd: useCallback(() => {
      setIsDragging(false);
    }, []),
  };

  const dropHandlers = {
    onDragOver: useCallback((e: React.DragEvent) => {
      e.preventDefault();
    }, []),
    onDragEnter: useCallback((e: React.DragEvent) => {
      e.preventDefault();
      setIsOver(true);
    }, []),
    onDragLeave: useCallback((e: React.DragEvent) => {
      void e;
      setIsOver(false);
    }, []),
    onDrop: useCallback(
      (e: React.DragEvent) => {
        e.preventDefault();
        setIsOver(false);
        try {
          const raw = e.dataTransfer.getData(MIME);
          const paths: string[] = raw ? JSON.parse(raw) : [];
          if (paths.length > 0) onDrop(paths);
        } catch {
          // invalid data, ignore
        }
      },
      [onDrop],
    ),
  };

  return { dragHandlers, dropHandlers, isDragging, isOver };
}

export { MIME as DRAG_MIME };
