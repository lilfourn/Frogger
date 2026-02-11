import { useCallback, useEffect } from "react";
import { create } from "zustand";
import type { FileEntry } from "../types/file";

interface NavState {
  focusIndex: number;
  setFocusIndex: (value: number) => void;
  moveDown: (max: number) => void;
  moveUp: () => void;
  reset: () => void;
}

export const useNavStore = create<NavState>()((set) => ({
  focusIndex: -1,
  setFocusIndex: (value) => set({ focusIndex: value }),
  moveDown: (max) => set((s) => ({ focusIndex: Math.min(s.focusIndex + 1, max) })),
  moveUp: () => set((s) => ({ focusIndex: Math.max(s.focusIndex - 1, 0) })),
  reset: () => set({ focusIndex: -1 }),
}));

export function useFileNavigation(entries: FileEntry[]) {
  const focusIndex = useNavStore((s) => s.focusIndex);
  const reset = useNavStore((s) => s.reset);

  useEffect(() => {
    reset();
  }, [entries, reset]);

  const moveDown = useCallback(() => {
    useNavStore.getState().moveDown(entries.length - 1);
  }, [entries.length]);

  const moveUp = useCallback(() => {
    useNavStore.getState().moveUp();
  }, []);

  const setFocusIndex = useCallback((value: number) => {
    useNavStore.getState().setFocusIndex(value);
  }, []);

  const focusedEntry = focusIndex >= 0 && focusIndex < entries.length ? entries[focusIndex] : null;

  return { focusIndex, setFocusIndex, moveDown, moveUp, focusedEntry };
}

export function resetFileNavigation() {
  useNavStore.getState().reset();
}
