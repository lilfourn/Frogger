import { useEffect, useCallback } from "react";
import { useChatStore } from "../stores/chatStore";
import { useFileStore } from "../stores/fileStore";
import { useTauriEvent } from "./useTauriEvents";
import {
  sendChat,
  getChatHistory,
  clearChatHistory,
  newChatSession,
  sendOrganizePlan,
  sendOrganizeExecute,
  sendOrganizeApply,
  cancelOrganize,
  getOrganizeStatus,
} from "../services/chatService";
import { openFile } from "../services/fileService";
import { parseOrganizePlan } from "../utils/actionParser";
import type { OrganizeProgress, OrganizeProgressPhase } from "../stores/chatStore";

interface StreamChunk {
  chunk: string;
  done: boolean;
}

type OrganizeProgressEvent = OrganizeProgress;

const ACTIVE_ORGANIZE_PROGRESS_PHASES: ReadonlySet<OrganizeProgressPhase> = new Set([
  "indexing",
  "planning",
  "applying",
]);

function normalizePath(path: string): string {
  return path.replace(/\/+$/, "");
}

function isSamePath(left: string, right: string): boolean {
  return normalizePath(left) === normalizePath(right);
}

function createClientSessionId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

const DB_LOCKED_ERROR_RE = /database(?: table)? is locked/i;
const ORGANIZE_DB_LOCKED_MESSAGE =
  "Frogger is busy finishing another file-index update. Please retry in a few seconds.";

function messageFromUnknownError(err: unknown): string | null {
  if (typeof err === "string") return err;
  if (err instanceof Error && typeof err.message === "string") return err.message;
  if (typeof err === "object" && err !== null) {
    const maybeMessage = (err as { message?: unknown }).message;
    if (typeof maybeMessage === "string") return maybeMessage;
  }
  return null;
}

function formatOrganizeError(err: unknown, fallback: string): string {
  const raw = messageFromUnknownError(err);
  if (!raw) return fallback;
  if (DB_LOCKED_ERROR_RE.test(raw)) {
    return ORGANIZE_DB_LOCKED_MESSAGE;
  }
  return raw;
}

export function useChat() {
  const sessionId = useChatStore((s) => s.sessionId);
  const setSessionId = useChatStore((s) => s.setSessionId);
  const setMessages = useChatStore((s) => s.setMessages);
  const addMessage = useChatStore((s) => s.addMessage);
  const setStreaming = useChatStore((s) => s.setStreaming);
  const appendStreamChunk = useChatStore((s) => s.appendStreamChunk);
  const commitStream = useChatStore((s) => s.commitStream);
  const setOrganize = useChatStore((s) => s.setOrganize);
  const setOrganizeProgress = useChatStore((s) => s.setOrganizeProgress);
  const resetOrganize = useChatStore((s) => s.resetOrganize);
  const clear = useChatStore((s) => s.clear);
  const organize = useChatStore((s) => s.organize);
  const currentPath = useFileStore((s) => s.currentPath);
  const selectedFiles = useFileStore((s) => s.selectedFiles);

  // Initialize session on mount
  useEffect(() => {
    if (!sessionId) {
      newChatSession()
        .then(setSessionId)
        .catch((err) => {
          console.error("[Chat] Session init failed:", err);
          addMessage({
            role: "assistant",
            content: "Failed to initialize chat session. Please try again.",
          });
        });
    }
  }, [sessionId, setSessionId, addMessage]);

  // Load history when session changes
  useEffect(() => {
    if (sessionId) {
      getChatHistory(sessionId)
        .then((records) => {
          setMessages(
            records.map((r) => ({
              id: r.id,
              role: r.role as "user" | "assistant",
              content: r.content,
              createdAt: r.created_at,
            })),
          );
        })
        .catch((err) => console.error("[Chat] Failed to load history:", err));
    }
  }, [sessionId, setMessages]);

  useTauriEvent<StreamChunk>("chat-stream", (payload) => {
    if (payload.done) {
      commitStream();
    } else {
      appendStreamChunk(payload.chunk);
    }
  });

  useTauriEvent<OrganizeProgressEvent>("organize-progress", (payload) => {
    const { organize } = useChatStore.getState();
    if (organize.phase === "idle") return;
    if (
      organize.folderPath &&
      payload.rootPath &&
      !isSamePath(organize.folderPath, payload.rootPath)
    )
      return;
    if (organize.progress?.sessionId && organize.progress.sessionId !== payload.sessionId) return;
    setOrganizeProgress(payload);
  });

  useEffect(() => {
    if (
      !organize.progress?.sessionId ||
      !ACTIVE_ORGANIZE_PROGRESS_PHASES.has(organize.progress.phase)
    ) {
      return;
    }

    let active = true;
    let inFlight = false;

    const pollStatus = () => {
      if (inFlight) return;
      const latest = useChatStore.getState().organize;
      if (
        !latest.progress?.sessionId ||
        !ACTIVE_ORGANIZE_PROGRESS_PHASES.has(latest.progress.phase)
      ) {
        return;
      }

      inFlight = true;
      getOrganizeStatus(latest.progress.sessionId)
        .then((payload: OrganizeProgressEvent | null) => {
          if (!active || !payload) return;
          const current = useChatStore.getState().organize;
          if (current.phase === "idle") return;
          if (
            current.folderPath &&
            payload.rootPath &&
            !isSamePath(current.folderPath, payload.rootPath)
          ) {
            return;
          }
          if (current.progress?.sessionId && payload.sessionId !== current.progress.sessionId) {
            return;
          }
          setOrganizeProgress(payload);
        })
        .catch((err: unknown) => {
          console.error("[Organize] Failed to poll progress status:", err);
        })
        .finally(() => {
          inFlight = false;
        });
    };

    pollStatus();
    const intervalId = window.setInterval(pollStatus, 900);

    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
  }, [
    organize.phase,
    organize.folderPath,
    organize.progress?.phase,
    organize.progress?.sessionId,
    setOrganizeProgress,
  ]);

  const send = useCallback(
    async (content: string) => {
      if (!content.trim() || !sessionId) return;
      addMessage({ role: "user", content });
      setStreaming(true);
      try {
        await sendChat(content, sessionId, currentPath, selectedFiles);
      } catch (err) {
        console.error("[Chat] send_chat failed:", err);
        if (useChatStore.getState().streamingContent) {
          commitStream();
        } else {
          const msg =
            typeof err === "string" ? err : "Something went wrong. Check your API key in Settings.";
          addMessage({ role: "assistant", content: msg });
          setStreaming(false);
        }
      }
    },
    [sessionId, currentPath, selectedFiles, addMessage, setStreaming, commitStream],
  );

  const startOrganize = useCallback(
    async (folderPath: string) => {
      const organizeSessionId = createClientSessionId();
      resetOrganize();
      setOrganize({ phase: "planning", folderPath });
      setOrganizeProgress({
        sessionId: organizeSessionId,
        rootPath: folderPath,
        phase: "indexing",
        processed: 0,
        total: 100,
        percent: 0,
        combinedPercent: 0,
        message: "Indexing directory tree...",
        sequence: 0,
      });
      try {
        const response = await sendOrganizePlan(folderPath, organizeSessionId);
        const currentSessionId = useChatStore.getState().organize.progress?.sessionId;
        if (currentSessionId && currentSessionId !== organizeSessionId) {
          return;
        }
        const plan = parseOrganizePlan(response);
        if (plan) {
          setOrganize({ phase: "plan-ready", plan, planRaw: response });
        } else {
          console.error("[Organize] Failed to parse plan:", response.slice(0, 500));
          setOrganize({ phase: "error", error: "Could not generate an organization plan." });
        }
      } catch (err) {
        const { organize } = useChatStore.getState();
        if (organize.phase === "idle" || organize.progress?.phase === "cancelled") return;
        console.error("[Organize] Plan request failed:", err);
        const msg = formatOrganizeError(err, "Failed to generate plan. Check your API key.");
        setOrganize({ phase: "error", error: msg });
      }
    },
    [setOrganize, setOrganizeProgress, resetOrganize],
  );

  const executeOrganize = useCallback(
    async (folderPath: string, planJson: string) => {
      const organizeSessionId =
        useChatStore.getState().organize.progress?.sessionId ?? createClientSessionId();
      const nextSequence = (useChatStore.getState().organize.progress?.sequence ?? 0) + 1;
      setOrganize({ phase: "executing", executeContent: "" });
      setOrganizeProgress({
        sessionId: organizeSessionId,
        rootPath: folderPath,
        phase: "applying",
        processed: 0,
        total: 1,
        percent: 0,
        combinedPercent: 85,
        message: "Preparing file operations...",
        sequence: nextSequence,
      });
      try {
        const response = await sendOrganizeExecute(folderPath, planJson, organizeSessionId);
        const currentSessionId = useChatStore.getState().organize.progress?.sessionId;
        if (currentSessionId && currentSessionId !== organizeSessionId) {
          return;
        }
        setOrganize({ phase: "complete", executeContent: response });
      } catch (err) {
        const { organize } = useChatStore.getState();
        if (organize.phase === "idle" || organize.progress?.phase === "cancelled") return;
        console.error("[Organize] Execute failed:", err);
        const msg = formatOrganizeError(err, "Failed to execute organization plan.");
        setOrganize({ phase: "error", error: msg });
      }
    },
    [setOrganize, setOrganizeProgress],
  );

  const applyOrganize = useCallback(
    async (folderPath: string, planJson: string) => {
      const organizeSessionId =
        useChatStore.getState().organize.progress?.sessionId ?? createClientSessionId();
      const nextSequence = (useChatStore.getState().organize.progress?.sequence ?? 0) + 1;
      setOrganizeProgress({
        sessionId: organizeSessionId,
        rootPath: folderPath,
        phase: "applying",
        processed: 0,
        total: 1,
        percent: 0,
        combinedPercent: 85,
        message: "Applying organization actions...",
        sequence: nextSequence,
      });
      try {
        await sendOrganizeApply(folderPath, planJson, organizeSessionId);
        setTimeout(() => {
          const { organize } = useChatStore.getState();
          if (organize.progress?.phase === "done") resetOrganize();
        }, 1500);
      } catch (err) {
        const { organize } = useChatStore.getState();
        if (organize.phase !== "idle" && organize.progress?.phase !== "cancelled") {
          console.error("[Organize] Apply failed:", err);
          const msg = formatOrganizeError(err, "Failed to apply organization actions.");
          setOrganize({ phase: "error", error: msg });
        }
        throw err;
      }
    },
    [setOrganize, setOrganizeProgress, resetOrganize],
  );

  const cancelActiveOrganize = useCallback(async () => {
    const { organize } = useChatStore.getState();
    if (!organize.folderPath) {
      resetOrganize();
      return;
    }
    try {
      await cancelOrganize(organize.folderPath, organize.progress?.sessionId);
    } catch (err) {
      console.error("[Organize] Cancel failed:", err);
    }
    resetOrganize();
  }, [resetOrganize]);

  const retryOrganize = useCallback(() => {
    const { organize: org } = useChatStore.getState();
    if (!org.folderPath) return;
    startOrganize(org.folderPath);
  }, [startOrganize]);

  const resetSession = useCallback(async () => {
    if (sessionId) {
      await clearChatHistory(sessionId).catch((err) =>
        console.error("[Chat] Failed to clear history:", err),
      );
    }
    clear();
    const id = await newChatSession();
    setSessionId(id);
  }, [sessionId, clear, setSessionId]);

  const openOrganizePath = useCallback(async (path: string) => {
    await openFile(path);
  }, []);

  return {
    send,
    startOrganize,
    executeOrganize,
    applyOrganize,
    cancelActiveOrganize,
    retryOrganize,
    resetSession,
    openOrganizePath,
  };
}
