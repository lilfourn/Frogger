import { create } from "zustand";
import type { PermissionCheckItem } from "../services/settingsService";
import { normalizePath } from "../utils/paths";

export type PermissionPromptDecision =
  | "deny"
  | "allow_once"
  | "always_allow_folder"
  | "always_allow_exact";

export interface PermissionPromptInput {
  title: string;
  action: string;
  promptKind: "initial" | "retry";
  blocked: PermissionCheckItem[];
  allowAlways: boolean;
  allowExactPath: boolean;
}

export const PERMISSION_PROMPT_TIMEOUT_MS = 120000;
const MAX_PERMISSION_PROMPT_QUEUE = 32;

interface PermissionPromptRequest extends PermissionPromptInput {
  id: number;
  key: string;
  timeout: ReturnType<typeof setTimeout>;
  resolvers: Array<(decision: PermissionPromptDecision) => void>;
}

interface PermissionPromptState {
  queue: PermissionPromptRequest[];
  queueIndexByKey: Record<string, number>;
  requestPrompt: (input: PermissionPromptInput) => Promise<PermissionPromptDecision>;
  resolveCurrent: (decision: PermissionPromptDecision) => void;
  cancelAll: () => void;
}

let nextPromptId = 1;

function promptKey(input: PermissionPromptInput): string {
  const blockedFingerprint = input.blocked
    .map((item) => {
      const path = normalizePath(item.path);
      const scopePath = item.scope_path ? normalizePath(item.scope_path) : "";
      return `${item.capability}:${path}:${scopePath}`;
    })
    .sort()
    .join("|");

  return [
    input.action,
    input.promptKind,
    input.allowAlways ? "allow-always" : "no-always",
    input.allowExactPath ? "allow-exact" : "no-exact",
    blockedFingerprint,
  ].join("::");
}

function mergeBlocked(
  existing: PermissionCheckItem[],
  incoming: PermissionCheckItem[],
): PermissionCheckItem[] {
  const seen = new Set<string>();
  const merged: PermissionCheckItem[] = [];

  for (const item of [...existing, ...incoming]) {
    const key = `${item.capability}:${normalizePath(item.path)}:${item.scope_path ?? ""}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    merged.push(item);
  }

  return merged;
}

function resolveRequest(
  request: PermissionPromptRequest,
  decision: PermissionPromptDecision,
): void {
  clearTimeout(request.timeout);
  for (const resolve of request.resolvers) {
    resolve(decision);
  }
}

function removeRequestById(
  state: Pick<PermissionPromptState, "queue" | "queueIndexByKey">,
  requestId: number,
): Pick<PermissionPromptState, "queue" | "queueIndexByKey"> {
  const request = state.queue.find((candidate) => candidate.id === requestId);
  if (!request) {
    return state;
  }

  const nextQueue = state.queue.filter((candidate) => candidate.id !== requestId);
  const nextIndexByKey = { ...state.queueIndexByKey };
  delete nextIndexByKey[request.key];
  return { queue: nextQueue, queueIndexByKey: nextIndexByKey };
}

function createTimeout(id: number): ReturnType<typeof setTimeout> {
  return setTimeout(() => {
    usePermissionPromptStore.setState((state) => {
      const request = state.queue.find((candidate) => candidate.id === id);
      if (!request) {
        return state;
      }

      resolveRequest(request, "deny");
      return removeRequestById(state, id);
    });
  }, PERMISSION_PROMPT_TIMEOUT_MS);
}

export const usePermissionPromptStore = create<PermissionPromptState>()((set, get) => ({
  queue: [],
  queueIndexByKey: {},
  requestPrompt: (input) =>
    new Promise<PermissionPromptDecision>((resolve) => {
      console.debug(`[PermissionPrompt] requestPrompt: action=${input.action}, title="${input.title}", queueLen=${get().queue.length}`);
      const key = promptKey(input);
      const existingId = get().queueIndexByKey[key];

      if (existingId) {
        set((state) => ({
          queue: state.queue.map((candidate) =>
            candidate.id === existingId
              ? {
                  ...candidate,
                  allowAlways: candidate.allowAlways || input.allowAlways,
                  allowExactPath: candidate.allowExactPath || input.allowExactPath,
                  blocked: mergeBlocked(candidate.blocked, input.blocked),
                  resolvers: [...candidate.resolvers, resolve],
                }
              : candidate,
          ),
        }));
        return;
      }

      const state = get();
      if (state.queue.length >= MAX_PERMISSION_PROMPT_QUEUE) {
        resolve("deny");
        return;
      }

      const id = nextPromptId;
      nextPromptId += 1;

      const request: PermissionPromptRequest = {
        id,
        key,
        title: input.title,
        action: input.action,
        promptKind: input.promptKind,
        blocked: input.blocked,
        allowAlways: input.allowAlways,
        allowExactPath: input.allowExactPath,
        timeout: createTimeout(id),
        resolvers: [resolve],
      };

      set((current) => ({
        queue: [...current.queue, request],
        queueIndexByKey: {
          ...current.queueIndexByKey,
          [key]: id,
        },
      }));
    }),
  resolveCurrent: (decision) => {
    const current = get().queue[0];
    if (!current) {
      console.debug("[PermissionPrompt] resolveCurrent: no current request");
      return;
    }

    console.debug(`[PermissionPrompt] resolveCurrent: decision=${decision}, action=${current.action}`);
    resolveRequest(current, decision);
    set((state) => removeRequestById(state, current.id));
  },
  cancelAll: () => {
    const queue = get().queue;
    for (const request of queue) {
      resolveRequest(request, "deny");
    }
    set({ queue: [], queueIndexByKey: {} });
  },
}));

export function requestPermissionPrompt(
  input: PermissionPromptInput,
): Promise<PermissionPromptDecision> {
  return usePermissionPromptStore.getState().requestPrompt(input);
}
