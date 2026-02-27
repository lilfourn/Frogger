import { describe, it, expect, beforeEach } from "vitest";
import { useChatStore } from "./chatStore";

describe("chatStore", () => {
  beforeEach(() => {
    useChatStore.setState(useChatStore.getInitialState());
  });

  it("starts with empty state", () => {
    const state = useChatStore.getState();
    expect(state.messages).toEqual([]);
    expect(state.isOpen).toBe(false);
    expect(state.isStreaming).toBe(false);
    expect(state.streamingContent).toBe("");
  });

  it("toggles open/close", () => {
    useChatStore.getState().toggle();
    expect(useChatStore.getState().isOpen).toBe(true);
    useChatStore.getState().toggle();
    expect(useChatStore.getState().isOpen).toBe(false);
  });

  it("adds messages", () => {
    useChatStore.getState().addMessage({ role: "user", content: "Hello" });
    useChatStore.getState().addMessage({ role: "assistant", content: "Hi" });
    expect(useChatStore.getState().messages).toHaveLength(2);
    expect(useChatStore.getState().messages[0].content).toBe("Hello");
    expect(useChatStore.getState().messages[1].role).toBe("assistant");
  });

  it("accumulates streaming content", () => {
    useChatStore.getState().setStreaming(true);
    useChatStore.getState().appendStreamChunk("Hello ");
    useChatStore.getState().appendStreamChunk("world");
    expect(useChatStore.getState().streamingContent).toBe("Hello world");
  });

  it("commits stream to messages", () => {
    useChatStore.getState().setStreaming(true);
    useChatStore.getState().appendStreamChunk("Response text");
    useChatStore.getState().commitStream();
    expect(useChatStore.getState().isStreaming).toBe(false);
    expect(useChatStore.getState().streamingContent).toBe("");
    expect(useChatStore.getState().messages).toHaveLength(1);
    expect(useChatStore.getState().messages[0].content).toBe("Response text");
    expect(useChatStore.getState().messages[0].role).toBe("assistant");
  });

  it("clears all state", () => {
    useChatStore.getState().addMessage({ role: "user", content: "test" });
    useChatStore.getState().setStreaming(true);
    useChatStore.getState().appendStreamChunk("chunk");
    useChatStore.getState().clear();
    expect(useChatStore.getState().messages).toEqual([]);
    expect(useChatStore.getState().streamingContent).toBe("");
    expect(useChatStore.getState().isStreaming).toBe(false);
  });

  it("sets session id", () => {
    useChatStore.getState().setSessionId("abc-123");
    expect(useChatStore.getState().sessionId).toBe("abc-123");
  });

  it("keeps organize progress monotonic by sequence", () => {
    const store = useChatStore.getState();

    store.setOrganizeProgress({
      sessionId: "organize-1",
      rootPath: "/tmp/project",
      phase: "indexing",
      processed: 10,
      total: 100,
      percent: 10,
      combinedPercent: 10,
      message: "Indexing 10/100",
      sequence: 2,
    });

    store.setOrganizeProgress({
      sessionId: "organize-1",
      rootPath: "/tmp/project",
      phase: "indexing",
      processed: 5,
      total: 100,
      percent: 5,
      combinedPercent: 5,
      message: "Stale",
      sequence: 1,
    });

    const afterStale = useChatStore.getState().organize.progress;
    expect(afterStale?.processed).toBe(10);
    expect(afterStale?.sequence).toBe(2);

    store.setOrganizeProgress({
      sessionId: "organize-1",
      rootPath: "/tmp/project",
      phase: "planning",
      processed: 40,
      total: 100,
      percent: 40,
      combinedPercent: 35,
      message: "Planning",
      sequence: 3,
    });

    store.setOrganizeProgress({
      sessionId: "organize-1",
      rootPath: "/tmp/project",
      phase: "indexing",
      processed: 90,
      total: 100,
      percent: 90,
      combinedPercent: 9,
      message: "Out-of-order phase",
      sequence: 4,
    });

    const latest = useChatStore.getState().organize.progress;
    expect(latest?.sequence).toBe(4);
    expect(latest?.combinedPercent).toBe(35);
  });

  it("ignores non-terminal updates after terminal organize phases", () => {
    const store = useChatStore.getState();

    store.setOrganizeProgress({
      sessionId: "organize-1",
      rootPath: "/tmp/project",
      phase: "done",
      processed: 100,
      total: 100,
      percent: 100,
      combinedPercent: 100,
      message: "Done",
      sequence: 10,
    });

    store.setOrganizeProgress({
      sessionId: "organize-1",
      rootPath: "/tmp/project",
      phase: "indexing",
      processed: 60,
      total: 100,
      percent: 60,
      combinedPercent: 6,
      message: "Late event",
      sequence: 11,
    });

    const progress = useChatStore.getState().organize.progress;
    expect(progress?.phase).toBe("done");
    expect(progress?.sequence).toBe(10);
  });
});
