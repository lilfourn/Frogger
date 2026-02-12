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
} from "../services/chatService";
import { parseOrganizePlan } from "../utils/actionParser";
import type { OrganizeProgress } from "../stores/chatStore";

interface StreamChunk {
  chunk: string;
  done: boolean;
}

type OrganizeProgressEvent = OrganizeProgress;

function createClientSessionId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
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
  const currentPath = useFileStore((s) => s.currentPath);
  const selectedFiles = useFileStore((s) => s.selectedFiles);

  // Initialize session on mount
  useEffect(() => {
    if (!sessionId) {
      newChatSession()
        .then(setSessionId)
        .catch((err) => {
          console.error("[Chat] Session init failed:", err);
          addMessage({ role: "assistant", content: "Failed to initialize chat session. Please try again." });
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
    if (organize.folderPath && payload.rootPath && organize.folderPath.replace(/\/+$/, "") !== payload.rootPath.replace(/\/+$/, "")) return;
    if (organize.progress?.sessionId && organize.progress.sessionId !== payload.sessionId) return;
    setOrganizeProgress(payload);
  });

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
      });
      try {
        const response = await sendOrganizePlan(folderPath, organizeSessionId);
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
        const msg = typeof err === "string" ? err : "Failed to generate plan. Check your API key.";
        setOrganize({ phase: "error", error: msg });
      }
    },
    [setOrganize, setOrganizeProgress, resetOrganize],
  );

  const executeOrganize = useCallback(
    async (folderPath: string, planJson: string) => {
      const organizeSessionId =
        useChatStore.getState().organize.progress?.sessionId ?? createClientSessionId();
      setOrganize({ phase: "executing", executeContent: "" });
      setOrganizeProgress({
        sessionId: organizeSessionId,
        rootPath: folderPath,
        phase: "applying",
        processed: 0,
        total: 1,
        percent: 0,
        combinedPercent: 70,
        message: "Preparing file operations...",
      });
      try {
        const response = await sendOrganizeExecute(folderPath, planJson, organizeSessionId);
        setOrganize({ phase: "complete", executeContent: response });
      } catch (err) {
        const { organize } = useChatStore.getState();
        if (organize.phase === "idle" || organize.progress?.phase === "cancelled") return;
        console.error("[Organize] Execute failed:", err);
        const msg = typeof err === "string" ? err : "Failed to execute organization plan.";
        setOrganize({ phase: "error", error: msg });
      }
    },
    [setOrganize, setOrganizeProgress],
  );

  const applyOrganize = useCallback(
    async (folderPath: string, planJson: string) => {
      const organizeSessionId =
        useChatStore.getState().organize.progress?.sessionId ?? createClientSessionId();
      setOrganizeProgress({
        sessionId: organizeSessionId,
        rootPath: folderPath,
        phase: "applying",
        processed: 0,
        total: 1,
        percent: 0,
        combinedPercent: 70,
        message: "Applying organization actions...",
      });
      try {
        await sendOrganizeApply(folderPath, planJson, organizeSessionId);
      } catch (err) {
        const { organize } = useChatStore.getState();
        if (organize.phase !== "idle" && organize.progress?.phase !== "cancelled") {
          console.error("[Organize] Apply failed:", err);
          const msg = typeof err === "string" ? err : "Failed to apply organization actions.";
          setOrganize({ phase: "error", error: msg });
        }
        throw err;
      }
    },
    [setOrganize, setOrganizeProgress],
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

  const resetSession = useCallback(async () => {
    if (sessionId) {
      await clearChatHistory(sessionId).catch((err) => console.error("[Chat] Failed to clear history:", err));
    }
    clear();
    const id = await newChatSession();
    setSessionId(id);
  }, [sessionId, clear, setSessionId]);

  return {
    send,
    startOrganize,
    executeOrganize,
    applyOrganize,
    cancelActiveOrganize,
    resetSession,
  };
}
