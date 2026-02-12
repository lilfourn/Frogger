import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ChatPanel } from "./ChatPanel";
import { useChatStore } from "../../stores/chatStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../../services/chatService", () => ({
  sendChat: vi.fn().mockResolvedValue("response"),
  getChatHistory: vi.fn().mockResolvedValue([]),
  clearChatHistory: vi.fn().mockResolvedValue(undefined),
  newChatSession: vi.fn().mockResolvedValue("test-session"),
  sendOrganizePlan: vi.fn().mockResolvedValue(""),
  sendOrganizeExecute: vi.fn().mockResolvedValue(""),
  sendOrganizeApply: vi.fn().mockResolvedValue(""),
  cancelOrganize: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../services/fileService", () => ({
  moveFiles: vi.fn().mockResolvedValue([]),
  copyFiles: vi.fn().mockResolvedValue([]),
  renameFile: vi.fn().mockResolvedValue(undefined),
  deleteFiles: vi.fn().mockResolvedValue(undefined),
  createDirectory: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../hooks/useChat", () => ({
  useChat: () => ({
    send: vi.fn(),
    startOrganize: vi.fn(),
    executeOrganize: vi.fn(),
    applyOrganize: vi.fn(),
    cancelActiveOrganize: vi.fn(),
    resetSession: vi.fn(),
  }),
}));

describe("ChatPanel", () => {
  beforeEach(() => {
    useChatStore.setState(useChatStore.getInitialState());
  });

  it("renders nothing when closed", () => {
    const { container } = render(<ChatPanel />);
    expect(container.querySelector("[data-testid='chat-panel']")).toBeNull();
  });

  it("renders panel when open", () => {
    useChatStore.getState().open();
    render(<ChatPanel />);
    expect(screen.getByTestId("chat-panel")).toBeInTheDocument();
  });

  it("shows empty state message", () => {
    useChatStore.getState().open();
    render(<ChatPanel />);
    expect(screen.getByText("Ask anything about your files")).toBeInTheDocument();
  });

  it("closes on close button click", () => {
    useChatStore.getState().open();
    render(<ChatPanel />);
    fireEvent.click(screen.getByTestId("chat-close"));
    expect(useChatStore.getState().isOpen).toBe(false);
  });

  it("renders messages", () => {
    useChatStore.setState({
      isOpen: true,
      messages: [
        { role: "user", content: "Hello" },
        { role: "assistant", content: "Hi there" },
      ],
    });
    render(<ChatPanel />);
    expect(screen.getByText("Hello")).toBeInTheDocument();
    expect(screen.getByText("Hi there")).toBeInTheDocument();
  });

  it("shows streaming indicator", () => {
    useChatStore.setState({
      isOpen: true,
      isStreaming: true,
      streamingContent: "",
    });
    render(<ChatPanel />);
    expect(screen.getByText("Thinking...")).toBeInTheDocument();
  });

  it("shows streaming content as it arrives", () => {
    useChatStore.setState({
      isOpen: true,
      isStreaming: true,
      streamingContent: "Partial response",
    });
    render(<ChatPanel />);
    expect(screen.getByText("Partial response")).toBeInTheDocument();
  });

  it("disables input while streaming", () => {
    useChatStore.setState({ isOpen: true, isStreaming: true });
    render(<ChatPanel />);
    expect(screen.getByTestId("chat-input")).toBeDisabled();
  });

  it("disables send when input is empty", () => {
    useChatStore.getState().open();
    render(<ChatPanel />);
    expect(screen.getByTestId("chat-send")).toBeDisabled();
  });

  it("renders tool confirmation for action blocks in assistant messages", () => {
    useChatStore.setState({
      isOpen: true,
      messages: [
        { role: "user", content: "Move my files" },
        {
          role: "assistant",
          content:
            'I\'ll move those for you:\n```action\n{"tool": "move_files", "args": {"sources": ["/a.txt"], "dest_dir": "/docs"}}\n```',
        },
      ],
    });
    render(<ChatPanel />);
    expect(screen.getByTestId("tool-confirmation")).toBeInTheDocument();
    expect(screen.getByText("Move 1 file → docs/")).toBeInTheDocument();
    expect(screen.getByTestId("tool-approve")).toBeInTheDocument();
    expect(screen.getByTestId("tool-deny")).toBeInTheDocument();
  });

  it("renders plain text for assistant messages without action blocks", () => {
    useChatStore.setState({
      isOpen: true,
      messages: [{ role: "assistant", content: "Here is some info about your files." }],
    });
    render(<ChatPanel />);
    expect(screen.getByText("Here is some info about your files.")).toBeInTheDocument();
    expect(screen.queryByTestId("tool-confirmation")).toBeNull();
  });
});
