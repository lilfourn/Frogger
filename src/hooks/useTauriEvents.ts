import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

export function useTauriEvent<T>(eventName: string, handler: (payload: T) => void) {
  const handlerRef = useRef(handler);

  useEffect(() => {
    handlerRef.current = handler;
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen<T>(eventName, (event) => {
      handlerRef.current(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [eventName]);
}
