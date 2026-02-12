import { useState, useEffect, useRef, useCallback } from "react";
import { Search, MessageSquare, FolderOpen, Key, Check, Send } from "lucide-react";
import { useOnboardingStore } from "../../stores/onboardingStore";
import { saveApiKey, hasApiKey } from "../../services/settingsService";
import { useChatStore } from "../../stores/chatStore";
import { useChat } from "../../hooks/useChat";
import appLogo from "../../assets/app-logo.svg";

const SUGGESTIONS = ["What files are in my home folder?", "Help me organize my Downloads"];

function ProgressDots({ current, completed }: { current: number; completed: boolean[] }) {
  return (
    <div className="flex items-center gap-1">
      {completed.map((done, i) => (
        <div key={i} className="flex items-center">
          {i > 0 && (
            <div
              className={`mx-0.5 h-0.5 w-4 ${done || completed[i - 1] ? "bg-[var(--color-accent)]" : "bg-[var(--color-border)]"}`}
            />
          )}
          <div
            className={`h-2.5 w-2.5 rounded-full transition-all duration-200 ${
              done
                ? "bg-[var(--color-accent)]"
                : i === current
                  ? "bg-[var(--color-accent)] ring-2 ring-[var(--color-accent)]/30"
                  : "bg-[var(--color-border)]"
            }`}
          />
        </div>
      ))}
    </div>
  );
}

function StepWelcome({ onNext }: { onNext: () => void }) {
  return (
    <div className="flex w-full max-w-[480px] flex-col items-center px-5 py-6">
      <img src={appLogo} alt="Frogger" className="h-12 w-12" />
      <h2 className="mt-3 text-lg font-semibold text-[var(--color-text)]">Welcome to Frogger</h2>
      <p className="mt-1 text-sm text-[var(--color-text-secondary)]">Your AI-native file manager</p>
      <div className="mt-4 flex gap-2">
        {[
          { icon: Search, label: "Smart Search" },
          { icon: MessageSquare, label: "AI Chat" },
          { icon: FolderOpen, label: "File Actions" },
        ].map(({ icon: Icon, label }) => (
          <span
            key={label}
            className="inline-flex items-center gap-1.5 rounded-full bg-[var(--color-bg-secondary)] px-3 py-1 text-xs text-[var(--color-text-secondary)]"
          >
            <Icon size={12} strokeWidth={1.5} />
            {label}
          </span>
        ))}
      </div>
      <button
        data-testid="onboarding-next"
        onClick={onNext}
        className="mt-6 w-full rounded bg-[var(--color-accent)] py-2 text-sm font-medium text-white"
      >
        Let's Go
      </button>
    </div>
  );
}

function StepApiKey({ onComplete }: { onComplete: () => void }) {
  const [keyInput, setKeyInput] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    hasApiKey()
      .then((exists) => {
        if (exists) {
          setSaved(true);
          onComplete();
        }
      })
      .catch((err) => console.error("[Onboarding] Failed to check API key:", err));
    inputRef.current?.focus();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  async function handleSave() {
    if (!keyInput.trim()) return;
    setSaving(true);
    try {
      await saveApiKey(keyInput.trim());
      setKeyInput("");
      setSaved(true);
      onComplete();
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex w-full max-w-[480px] flex-col items-center px-5 py-6">
      <div className="flex h-10 w-10 items-center justify-center rounded-full bg-[var(--color-bg-secondary)]">
        <Key size={24} strokeWidth={1.5} className="text-[var(--color-accent)]" />
      </div>
      <h2 className="mt-3 text-lg font-semibold text-[var(--color-text)]">Connect Your Brain</h2>
      <p className="mt-1 text-sm text-[var(--color-text-secondary)]">
        Add your Anthropic API key to enable AI features
      </p>

      {saved ? (
        <div className="mt-4 flex items-center gap-2 text-sm text-[var(--color-accent)]">
          <Check size={16} strokeWidth={2} /> API key connected
        </div>
      ) : (
        <>
          <div className="mt-4 w-full">
            <input
              ref={inputRef}
              data-testid="onboarding-api-input"
              type="password"
              value={keyInput}
              onChange={(e) => setKeyInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSave();
              }}
              placeholder="sk-ant-..."
              className="w-full rounded border border-[var(--color-border)] bg-transparent px-3 py-1.5 text-sm outline-none placeholder:text-[var(--color-text-secondary)]"
            />
          </div>
          <button
            data-testid="onboarding-save-key"
            onClick={handleSave}
            disabled={!keyInput.trim() || saving}
            className="mt-3 w-full rounded bg-[var(--color-accent)] py-2 text-sm font-medium text-white disabled:opacity-50"
          >
            Save Key
          </button>
          <a
            href="https://console.anthropic.com/settings/keys"
            target="_blank"
            rel="noopener noreferrer"
            className="mt-2 text-xs text-[var(--color-accent)] hover:underline"
          >
            Get an API key from Anthropic
          </a>
        </>
      )}
    </div>
  );
}

function StepChat({ onComplete }: { onComplete: () => void }) {
  const messages = useChatStore((s) => s.messages);
  const isStreaming = useChatStore((s) => s.isStreaming);
  const streamingContent = useChatStore((s) => s.streamingContent);
  const { send } = useChat();
  const [input, setInput] = useState("");
  const completedRef = useRef(false);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (listRef.current?.scrollTo) {
      listRef.current.scrollTo({ top: listRef.current.scrollHeight });
    }
  }, [messages, streamingContent]);

  useEffect(() => {
    if (messages.some((m) => m.role === "assistant") && !completedRef.current) {
      completedRef.current = true;
      onComplete();
    }
  }, [messages, onComplete]);

  function handleSend(text: string) {
    if (!text.trim() || isStreaming) return;
    send(text.trim());
    setInput("");
  }

  return (
    <div className="flex w-full max-w-[480px] flex-col px-5 py-6">
      <div className="text-center">
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Say Hello</h2>
        <p className="mt-1 text-sm text-[var(--color-text-secondary)]">Try sending a message</p>
      </div>

      <div
        ref={listRef}
        className="mt-3 h-[140px] space-y-2 overflow-y-auto rounded border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-2"
      >
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`rounded px-2.5 py-1.5 text-xs ${
              msg.role === "user"
                ? "ml-6 bg-[var(--color-accent)] text-white"
                : "mr-6 bg-[var(--color-bg)] text-[var(--color-text)]"
            }`}
          >
            {msg.content}
          </div>
        ))}
        {isStreaming && (
          <div className="mr-6 rounded bg-[var(--color-bg)] px-2.5 py-1.5 text-xs text-[var(--color-text)]">
            {streamingContent || (
              <span className="text-[var(--color-text-secondary)]">Thinking...</span>
            )}
          </div>
        )}
      </div>

      {!messages.some((m) => m.role === "assistant") && !isStreaming && messages.length === 0 && (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {SUGGESTIONS.map((s) => (
            <button
              key={s}
              data-testid="onboarding-suggestion"
              onClick={() => handleSend(s)}
              className="rounded-full border border-[var(--color-border)] px-3 py-1 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
            >
              {s}
            </button>
          ))}
        </div>
      )}

      <div className="mt-2 flex items-center gap-2">
        <input
          data-testid="onboarding-chat-input"
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              handleSend(input);
            }
          }}
          placeholder="Type a message..."
          disabled={isStreaming}
          className="w-full rounded border border-[var(--color-border)] bg-transparent px-3 py-1.5 text-sm outline-none placeholder:text-[var(--color-text-secondary)] disabled:opacity-50"
        />
        <button
          onClick={() => handleSend(input)}
          disabled={!input.trim() || isStreaming}
          className="rounded bg-[var(--color-accent)] p-1.5 text-white disabled:opacity-50"
        >
          <Send size={14} strokeWidth={1.5} />
        </button>
      </div>
    </div>
  );
}

function StepComplete({ onFinish }: { onFinish: () => void }) {
  const [showCheck, setShowCheck] = useState(false);

  useEffect(() => {
    const t = setTimeout(() => setShowCheck(true), 100);
    return () => clearTimeout(t);
  }, []);

  return (
    <div className="flex w-full max-w-[480px] flex-col items-center px-5 py-6">
      <div
        className={`flex h-14 w-14 items-center justify-center rounded-full bg-[var(--color-accent)] text-white transition-transform duration-300 ease-out ${showCheck ? "scale-100" : "scale-0"}`}
      >
        <Check size={28} strokeWidth={2} />
      </div>
      <h2 className="mt-3 text-lg font-semibold text-[var(--color-text)]">You're Ready!</h2>
      <p className="mt-1 text-sm text-[var(--color-text-secondary)]">
        Cmd+Shift+C to open chat anytime
      </p>
      <button
        data-testid="onboarding-finish"
        onClick={onFinish}
        className="mt-6 w-full rounded bg-[var(--color-accent)] py-2 text-sm font-medium text-white"
      >
        Start Exploring
      </button>
    </div>
  );
}

export function OnboardingModal() {
  const isActive = useOnboardingStore((s) => s.isActive);
  const currentStep = useOnboardingStore((s) => s.currentStep);
  const completedSteps = useOnboardingStore((s) => s.completedSteps);
  const completeStep = useOnboardingStore((s) => s.completeStep);
  const nextStep = useOnboardingStore((s) => s.nextStep);
  const finish = useOnboardingStore((s) => s.finish);

  const handleWelcomeNext = useCallback(() => {
    completeStep(0);
    nextStep();
  }, [completeStep, nextStep]);

  const handleApiComplete = useCallback(() => {
    completeStep(1);
    nextStep();
  }, [completeStep, nextStep]);

  const handleChatComplete = useCallback(() => {
    completeStep(2);
    nextStep();
  }, [completeStep, nextStep]);

  const handleFinish = useCallback(() => {
    completeStep(3);
    finish();
  }, [completeStep, finish]);

  if (!isActive) return null;

  return (
    <div
      data-testid="onboarding-modal"
      className="fixed inset-0 z-50 flex flex-col bg-[var(--color-bg)]"
    >
      {/* Header with progress */}
      <div className="flex items-center justify-center border-b border-[var(--color-border)] px-5 py-3">
        <ProgressDots current={currentStep} completed={completedSteps} />
      </div>

      {/* Step content — centered */}
      <div className="flex flex-1 items-center justify-center">
        {currentStep === 0 && <StepWelcome onNext={handleWelcomeNext} />}
        {currentStep === 1 && <StepApiKey onComplete={handleApiComplete} />}
        {currentStep === 2 && <StepChat onComplete={handleChatComplete} />}
        {currentStep === 3 && <StepComplete onFinish={handleFinish} />}
      </div>

      {/* Skip */}
      {currentStep < 3 && (
        <div className="border-t border-[var(--color-border)] px-5 py-2">
          <button
            data-testid="onboarding-skip"
            onClick={finish}
            className="text-xs text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
          >
            Skip
          </button>
        </div>
      )}
    </div>
  );
}
