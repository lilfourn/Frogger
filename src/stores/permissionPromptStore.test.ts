import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import {
  PERMISSION_PROMPT_TIMEOUT_MS,
  type PermissionPromptInput,
  usePermissionPromptStore,
} from "./permissionPromptStore";

function promptInput(overrides: Partial<PermissionPromptInput> = {}): PermissionPromptInput {
  return {
    title: "Permission",
    action: "list_directory",
    promptKind: "initial" as const,
    blocked: [],
    allowAlways: false,
    allowExactPath: false,
    ...overrides,
  };
}

describe("permissionPromptStore", () => {
  beforeEach(() => {
    usePermissionPromptStore.getState().cancelAll();
    usePermissionPromptStore.setState(usePermissionPromptStore.getInitialState());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("queues prompts and resolves in order", async () => {
    const first = usePermissionPromptStore
      .getState()
      .requestPrompt(promptInput({ title: "First" }));
    const second = usePermissionPromptStore
      .getState()
      .requestPrompt(promptInput({ title: "Second", action: "search" }));

    expect(usePermissionPromptStore.getState().queue).toHaveLength(2);

    usePermissionPromptStore.getState().resolveCurrent("allow_once");
    await expect(first).resolves.toBe("allow_once");
    expect(usePermissionPromptStore.getState().queue).toHaveLength(1);

    usePermissionPromptStore.getState().resolveCurrent("deny");
    await expect(second).resolves.toBe("deny");
    expect(usePermissionPromptStore.getState().queue).toHaveLength(0);
  });

  it("deduplicates identical prompts and resolves all listeners", async () => {
    const input = promptInput({
      blocked: [
        {
          path: "/Users/test",
          capability: "content_scan",
          mode: "ask" as const,
          scope_path: null,
        },
      ],
      allowAlways: true,
      allowExactPath: true,
    });

    const first = usePermissionPromptStore.getState().requestPrompt(input);
    const second = usePermissionPromptStore.getState().requestPrompt(input);

    expect(usePermissionPromptStore.getState().queue).toHaveLength(1);

    usePermissionPromptStore.getState().resolveCurrent("always_allow_folder");
    await expect(first).resolves.toBe("always_allow_folder");
    await expect(second).resolves.toBe("always_allow_folder");
    expect(usePermissionPromptStore.getState().queue).toHaveLength(0);
  });

  it("does not dedupe different actions", async () => {
    const first = usePermissionPromptStore
      .getState()
      .requestPrompt(promptInput({ title: "Permission", action: "search" }));
    const second = usePermissionPromptStore
      .getState()
      .requestPrompt(promptInput({ title: "Permission", action: "list_directory" }));

    expect(usePermissionPromptStore.getState().queue).toHaveLength(2);

    usePermissionPromptStore.getState().resolveCurrent("deny");
    usePermissionPromptStore.getState().resolveCurrent("deny");
    await expect(first).resolves.toBe("deny");
    await expect(second).resolves.toBe("deny");
  });

  it("expires queued prompts and resolves deny", async () => {
    vi.useFakeTimers();
    const pending = usePermissionPromptStore
      .getState()
      .requestPrompt(promptInput({ title: "Timeout" }));

    expect(usePermissionPromptStore.getState().queue).toHaveLength(1);

    vi.advanceTimersByTime(PERMISSION_PROMPT_TIMEOUT_MS + 1);

    await expect(pending).resolves.toBe("deny");
    expect(usePermissionPromptStore.getState().queue).toHaveLength(0);
  });

  it("cancels all queued prompts as deny", async () => {
    const first = usePermissionPromptStore.getState().requestPrompt(promptInput({ title: "A" }));
    const second = usePermissionPromptStore.getState().requestPrompt(promptInput({ title: "B" }));

    usePermissionPromptStore.getState().cancelAll();

    await expect(first).resolves.toBe("deny");
    await expect(second).resolves.toBe("deny");
    expect(usePermissionPromptStore.getState().queue).toHaveLength(0);
  });

  it("rejects new prompts when queue is saturated", async () => {
    for (let i = 0; i < 32; i += 1) {
      usePermissionPromptStore
        .getState()
        .requestPrompt(promptInput({ action: `action_${i}`, title: `Prompt ${i}` }));
    }

    const overflow = usePermissionPromptStore
      .getState()
      .requestPrompt(promptInput({ action: "overflow_action", title: "Overflow" }));

    expect(usePermissionPromptStore.getState().queue).toHaveLength(32);
    await expect(overflow).resolves.toBe("deny");
  });
});
