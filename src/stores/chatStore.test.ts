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
});
