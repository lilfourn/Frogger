import { useState, useRef, useEffect, useCallback } from "react";
import { MessageSquare, X, RotateCcw, Send, Zap, Shield } from "lucide-react";
import { useChatStore } from "../../stores/chatStore";
import { useChat } from "../../hooks/useChat";
import { useFileActions } from "../../hooks/useFileActions";
import { ToolConfirmation } from "./ToolConfirmation";
import { DiffPreview } from "./DiffPreview";
import { parseActionBlocks } from "../../utils/actionParser";
import type { FileAction } from "../../utils/actionParser";

interface AssistantMessageProps {
  content: string;
  onApprove: (action: FileAction) => void;
  onDeny: (action: FileAction) => void;
  onApproveAll: (actions: FileAction[]) => void;
}

function AssistantMessage({ content, onApprove, onDeny, onApproveAll }: AssistantMessageProps) {
  const segments = parseActionBlocks(content);
  const actions = segments.filter((s) => s.type === "action" && s.action).map((s) => s.action!);

  const hasMultipleActions = actions.length > 1;

  return (
    <div className="space-y-2 rounded-lg bg-[var(--color-bg-secondary)] px-3 py-2 text-sm text-[var(--color-text)]">
      {segments.map((seg, i) => {
        if (seg.type === "text") {
          return (
            <p key={i} className="whitespace-pre-wrap">
              {seg.content}
            </p>
          );
        }
        if (hasMultipleActions) return null;
        return (
          <ToolConfirmation key={i} action={seg.action!} onApprove={onApprove} onDeny={onDeny} />
        );
      })}
      {hasMultipleActions && (
        <DiffPreview
          actions={actions}
          onApproveAll={onApproveAll}
          onDenyAll={() => actions.forEach(onDeny)}
        />
      )}
    </div>
  );
}

export function ChatPanel() {
  const messages = useChatStore((s) => s.messages);
  const isStreaming = useChatStore((s) => s.isStreaming);
  const isOpen = useChatStore((s) => s.isOpen);
  const streamingContent = useChatStore((s) => s.streamingContent);
  const close = useChatStore((s) => s.close);
  const approvalMode = useChatStore((s) => s.approvalMode);
  const setApprovalMode = useChatStore((s) => s.setApprovalMode);
  const setPendingInput = useChatStore((s) => s.setPendingInput);
  const { send, resetSession } = useChat();
  const { execute } = useFileActions();
  const [input, setInput] = useState("");
  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleApprove = useCallback(
    async (action: FileAction) => {
      await execute(action);
    },
    [execute],
  );

  const handleDeny: (action: FileAction) => void = useCallback((action: FileAction) => {
    console.warn("[Chat] Action denied:", action.tool);
  }, []);

  const handleApproveAll = useCallback(
    async (actions: FileAction[]) => {
      for (const action of actions) {
        await execute(action);
      }
    },
    [execute],
  );

  useEffect(() => {
    if (listRef.current?.scrollTo) {
      listRef.current.scrollTo({ top: listRef.current.scrollHeight });
    }
  }, [messages, streamingContent]);

  useEffect(() => {
    if (!isOpen) return;
    inputRef.current?.focus();
    const pending = useChatStore.getState().pendingInput;
    if (!pending) return;
    setPendingInput("");
    send(pending);
  }, [isOpen, setPendingInput, send]);

  if (!isOpen) return null;

  function handleSend() {
    if (!input.trim() || isStreaming) return;
    send(input.trim());
    setInput("");
  }

  return (
    <div
      data-testid="chat-panel"
      className="flex h-full w-[360px] flex-shrink-0 flex-col border-l border-[var(--color-border)] bg-[var(--color-bg)]"
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-3 py-[5.2px]">
        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <MessageSquare size={13} strokeWidth={1.5} />
          <span>Chat</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            data-testid="chat-mode-toggle"
            onClick={() => setApprovalMode(approvalMode === "suggest" ? "auto" : "suggest")}
            title={
              approvalMode === "suggest"
                ? "Suggest mode (click to switch to auto)"
                : "Auto mode (click to switch to suggest)"
            }
            className={`rounded px-1.5 py-0.5 ${approvalMode === "auto" ? "text-[var(--color-accent)]" : "text-[var(--color-text-secondary)]"} hover:bg-[var(--color-border)]`}
          >
            {approvalMode === "auto" ? (
              <Zap size={13} strokeWidth={1.5} />
            ) : (
              <Shield size={13} strokeWidth={1.5} />
            )}
          </button>
          <button
            data-testid="chat-reset"
            onClick={resetSession}
            aria-label="New session"
            className="rounded px-1.5 py-0.5 text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]"
          >
            <RotateCcw size={13} strokeWidth={1.5} />
          </button>
          <button
            data-testid="chat-close"
            onClick={close}
            aria-label="Close chat"
            className="rounded px-1.5 py-0.5 text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]"
          >
            <X size={14} strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Messages */}
      <div
        ref={listRef}
        data-testid="chat-messages"
        className="flex-1 space-y-3 overflow-y-auto px-3 py-2"
      >
        {messages.map((msg, i) => (
          <div key={i} className={msg.role === "user" ? "ml-8" : "mr-8"}>
            {msg.role === "user" ? (
              <div className="rounded-lg bg-[var(--color-accent)] px-3 py-2 text-sm text-white">
                {msg.content}
              </div>
            ) : (
              <AssistantMessage
                content={msg.content}
                onApprove={handleApprove}
                onDeny={handleDeny}
                onApproveAll={handleApproveAll}
              />
            )}
          </div>
        ))}

        {isStreaming && (
          <div className="mr-8">
            <div className="rounded-lg bg-[var(--color-bg-secondary)] px-3 py-2 text-sm text-[var(--color-text)]">
              {streamingContent || (
                <span className="flex items-center gap-1.5 text-[var(--color-text-secondary)]">
                  <svg className="h-3 w-3 animate-spin" viewBox="0 0 16 16" fill="none">
                    <circle
                      cx="8"
                      cy="8"
                      r="6"
                      stroke="currentColor"
                      strokeWidth="2"
                      opacity="0.3"
                    />
                    <path
                      d="M14 8a6 6 0 0 0-6-6"
                      stroke="currentColor"
                      strokeWidth="2"
                      strokeLinecap="round"
                    />
                  </svg>
                  Thinking...
                </span>
              )}
            </div>
          </div>
        )}

        {messages.length === 0 && !isStreaming && (
          <div className="flex h-full items-center justify-center text-xs text-[var(--color-text-secondary)]">
            Ask anything about your files
          </div>
        )}
      </div>

      {/* Input */}
      <div className="border-t border-[var(--color-border)] px-3 py-2">
        <div className="flex items-center gap-2">
          <input
            ref={inputRef}
            data-testid="chat-input"
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
            placeholder="Message Frogger..."
            disabled={isStreaming}
            className="w-full rounded border border-[var(--color-border)] bg-transparent px-3 py-1.5 text-sm outline-none placeholder:text-[var(--color-text-secondary)] disabled:opacity-50"
          />
          <button
            data-testid="chat-send"
            onClick={handleSend}
            disabled={!input.trim() || isStreaming}
            aria-label="Send message"
            className="rounded bg-[var(--color-accent)] p-1.5 text-white disabled:opacity-50"
          >
            <Send size={14} strokeWidth={1.5} />
          </button>
        </div>
      </div>
    </div>
  );
}
