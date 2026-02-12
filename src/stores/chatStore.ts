import { create } from "zustand";

import type { OrganizePlan } from "../utils/actionParser";

export interface ChatMessage {
  id?: number;
  role: "user" | "assistant";
  content: string;
  createdAt?: string;
}

export type OrganizePhase = "idle" | "planning" | "plan-ready" | "executing" | "complete" | "error";
export type OrganizeProgressPhase =
  | "indexing"
  | "planning"
  | "applying"
  | "done"
  | "cancelled"
  | "error";

export interface OrganizeProgress {
  sessionId: string;
  rootPath: string;
  phase: OrganizeProgressPhase;
  processed: number;
  total: number;
  percent: number;
  combinedPercent: number;
  message: string;
}

export interface OrganizeState {
  phase: OrganizePhase;
  folderPath: string;
  plan: OrganizePlan | null;
  planRaw: string;
  executeContent: string;
  error: string;
  progress: OrganizeProgress | null;
}

export type ApprovalMode = "suggest" | "auto";

const INITIAL_ORGANIZE: OrganizeState = {
  phase: "idle",
  folderPath: "",
  plan: null,
  planRaw: "",
  executeContent: "",
  error: "",
  progress: null,
};

interface ChatState {
  messages: ChatMessage[];
  sessionId: string;
  isStreaming: boolean;
  isOpen: boolean;
  streamingContent: string;
  approvalMode: ApprovalMode;
  pendingInput: string;
  organize: OrganizeState;
  setSessionId: (id: string) => void;
  setMessages: (messages: ChatMessage[]) => void;
  addMessage: (message: ChatMessage) => void;
  setStreaming: (streaming: boolean) => void;
  appendStreamChunk: (chunk: string) => void;
  commitStream: () => void;
  setApprovalMode: (mode: ApprovalMode) => void;
  setPendingInput: (input: string) => void;
  setOrganize: (update: Partial<OrganizeState>) => void;
  setOrganizeProgress: (progress: OrganizeProgress) => void;
  clearOrganizeProgress: () => void;
  resetOrganize: () => void;
  open: () => void;
  close: () => void;
  toggle: () => void;
  clear: () => void;
}

export const useChatStore = create<ChatState>()((set, get) => ({
  messages: [],
  sessionId: "",
  isStreaming: false,
  isOpen: false,
  streamingContent: "",
  approvalMode: "suggest",
  pendingInput: "",
  organize: { ...INITIAL_ORGANIZE },
  setSessionId: (sessionId) => set({ sessionId }),
  setMessages: (messages) => set({ messages }),
  addMessage: (message) => set((s) => ({ messages: [...s.messages, message] })),
  setStreaming: (isStreaming) =>
    set({ isStreaming, streamingContent: isStreaming ? "" : get().streamingContent }),
  appendStreamChunk: (chunk) => set((s) => ({ streamingContent: s.streamingContent + chunk })),
  commitStream: () =>
    set((s) => ({
      messages: [...s.messages, { role: "assistant" as const, content: s.streamingContent }],
      streamingContent: "",
      isStreaming: false,
    })),
  setApprovalMode: (approvalMode) => set({ approvalMode }),
  setPendingInput: (pendingInput) => set({ pendingInput }),
  setOrganize: (update) => set((s) => ({ organize: { ...s.organize, ...update } })),
  setOrganizeProgress: (progress) => set((s) => ({ organize: { ...s.organize, progress } })),
  clearOrganizeProgress: () => set((s) => ({ organize: { ...s.organize, progress: null } })),
  resetOrganize: () => set({ organize: { ...INITIAL_ORGANIZE } }),
  open: () => set({ isOpen: true }),
  close: () => set({ isOpen: false }),
  toggle: () => set((s) => ({ isOpen: !s.isOpen })),
  clear: () => set({ messages: [], streamingContent: "", isStreaming: false }),
}));
