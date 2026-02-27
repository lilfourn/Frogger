import { useState, useEffect, useRef } from "react";
import { Settings, X } from "lucide-react";
import {
  clearIndexedData,
  deleteApiKey,
  getReembedStatus,
  hasApiKey,
  startIndexing,
  startReembedIndexedFiles,
  stopIndexing,
  saveApiKey,
  type ReembedProgressState,
} from "../../services/settingsService";
import { getHomeDir } from "../../services/fileService";
import { PermissionSettings } from "./PermissionSettings";
import { PrivacyLogViewer } from "./PrivacyLogViewer";
import { useTauriEvent } from "../../hooks/useTauriEvents";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
  const [tab, setTab] = useState<"general" | "permissions" | "privacy">("general");
  const [anthropicKeyInput, setAnthropicKeyInput] = useState("");
  const [anthropicKeyExists, setAnthropicKeyExists] = useState(false);
  const [saving, setSaving] = useState(false);
  const [reembedProgress, setReembedProgress] = useState<ReembedProgressState | null>(null);
  const [clearingIndex, setClearingIndex] = useState(false);
  const [clearIndexMessage, setClearIndexMessage] = useState<string | null>(null);
  const [reindexing, setReindexing] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useTauriEvent<ReembedProgressState>("reembed-progress", (payload) => {
    setReembedProgress(payload);
  });

  useEffect(() => {
    if (isOpen) {
      Promise.all([hasApiKey(), getReembedStatus()])
        .then(([exists, reembedStatus]) => {
          setAnthropicKeyExists(exists);
          setReembedProgress(reembedStatus);
        })
        .catch((err) => console.error("[Settings] Failed to load settings:", err));
      setAnthropicKeyInput("");
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  async function handleSave() {
    if (!anthropicKeyInput.trim()) return;
    setSaving(true);
    try {
      await saveApiKey(anthropicKeyInput.trim());
      setAnthropicKeyExists(true);
      setAnthropicKeyInput("");
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete() {
    await deleteApiKey();
    setAnthropicKeyExists(false);
  }

  async function handleReembedNow() {
    try {
      const status = await startReembedIndexedFiles();
      setReembedProgress(status);
    } catch (err) {
      console.error("[Settings] Re-embed failed:", err);
      setReembedProgress({
        status: "error",
        processed: reembedProgress?.processed ?? 0,
        total: reembedProgress?.total ?? 0,
        embedded: reembedProgress?.embedded ?? 0,
        skipped_missing: reembedProgress?.skipped_missing ?? 0,
        failed: reembedProgress?.failed ?? 0,
        message: "Failed to start embedding rebuild.",
      });
    }
  }

  async function handleClearIndexedData() {
    setClearingIndex(true);
    setClearIndexMessage(null);
    try {
      const report = await clearIndexedData();
      setClearIndexMessage(`Cleared ${report.files_removed} files, ${report.vec_removed} vectors.`);
    } catch (err) {
      console.error("[Settings] Clear indexed data failed:", err);
      setClearIndexMessage("Failed to clear indexed data.");
    } finally {
      setClearingIndex(false);
    }
  }

  async function handleReindex() {
    setReindexing(true);
    setClearIndexMessage(null);
    try {
      await stopIndexing();
      const homeDir = await getHomeDir();
      if (homeDir) {
        await startIndexing(homeDir);
        setClearIndexMessage("Reindexing started.");
      }
    } catch (err) {
      console.error("[Settings] Reindex failed:", err);
      setClearIndexMessage("Failed to start reindex.");
    } finally {
      setReindexing(false);
    }
  }

  const isReembedding = reembedProgress?.status === "running";
  const isIndexBusy = clearingIndex || reindexing;

  return (
    <div
      data-testid="settings-overlay"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      <div
        className="w-full max-w-[420px] rounded-lg bg-[var(--color-bg)] shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-[var(--color-border)] px-4 py-2">
          <div className="flex items-center gap-2">
            <Settings size={15} strokeWidth={1.5} className="text-[var(--color-text-secondary)]" />
            <span className="text-sm font-medium">Settings</span>
          </div>
          <button
            onClick={onClose}
            aria-label="Close settings"
            className="rounded p-1 text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
          >
            <X size={16} strokeWidth={1.5} />
          </button>
        </div>

        <div className="flex gap-2 border-b border-[var(--color-border)] px-4">
          {(["general", "permissions", "privacy"] as const).map((t) => (
            <button
              key={t}
              data-testid={`tab-${t}`}
              onClick={() => setTab(t)}
              className={`pb-2 pt-3 text-xs capitalize ${
                tab === t
                  ? "border-b-2 border-[var(--color-text)] font-medium text-[var(--color-text)]"
                  : "text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
              }`}
            >
              {t}
            </button>
          ))}
        </div>

        <div className="space-y-6 px-4 py-4">
          {tab === "general" && (
            <div className="flex flex-col gap-6">
              <div>
                <div className="mb-2 flex items-center gap-2">
                  <h3 className="text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
                    Anthropic API Key
                  </h3>
                  <span
                    data-testid="key-status"
                    className={`h-1.5 w-1.5 rounded-full ${anthropicKeyExists ? "bg-green-500" : "bg-[var(--color-border)]"}`}
                  />
                </div>

                <div className="flex gap-2">
                  <input
                    ref={inputRef}
                    data-testid="api-key-input"
                    type="password"
                    value={anthropicKeyInput}
                    onChange={(e) => setAnthropicKeyInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleSave();
                    }}
                    placeholder={
                      anthropicKeyExists ? "Key saved — enter new key to replace" : "sk-ant-..."
                    }
                    className="flex-1 rounded border border-[var(--color-border)] bg-transparent px-3 py-1.5 text-sm outline-none transition-colors focus:border-[var(--color-text)] placeholder:text-[var(--color-text-secondary)]"
                  />
                  <button
                    data-testid="save-key-btn"
                    onClick={handleSave}
                    disabled={!anthropicKeyInput.trim() || saving}
                    className="rounded bg-[var(--color-text)] px-3 py-1.5 text-sm font-medium text-[var(--color-bg)] transition-opacity hover:opacity-90 disabled:opacity-50"
                  >
                    Save
                  </button>
                  {anthropicKeyExists && (
                    <button
                      data-testid="delete-key-btn"
                      onClick={handleDelete}
                      className="rounded px-3 py-1.5 text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-secondary)] hover:text-[var(--color-text)]"
                    >
                      Remove
                    </button>
                  )}
                </div>
              </div>

              <div className="border-t border-[var(--color-border)] pt-6">
                <h3 className="text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
                  Semantic Embeddings
                </h3>
                <p className="mt-2 text-xs leading-relaxed text-[var(--color-text-secondary)]">
                  Local embeddings powered by BGE-small-en-v1.5 (384 dims). All processing happens
                  on-device.
                </p>
                <div className="mt-4 flex items-center justify-between">
                  <button
                    data-testid="reembed-now-btn"
                    onClick={handleReembedNow}
                    disabled={isReembedding}
                    className="rounded border border-[var(--color-border)] px-3 py-1.5 text-xs font-medium transition-colors hover:bg-[var(--color-bg-secondary)] hover:text-[var(--color-text)] disabled:opacity-50"
                  >
                    {isReembedding ? "Rebuilding..." : "Rebuild Embeddings"}
                  </button>
                  {reembedProgress && (
                    <span
                      data-testid="reembed-summary"
                      className="text-xs text-[var(--color-text-secondary)]"
                    >
                      {reembedProgress.message}
                      {reembedProgress.total > 0 &&
                        ` (${reembedProgress.processed}/${reembedProgress.total})`}
                    </span>
                  )}
                </div>
              </div>

              <div className="border-t border-[var(--color-border)] pt-6">
                <h3 className="text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
                  Index Management
                </h3>
                <div className="mt-3 flex items-center gap-2">
                  <button
                    data-testid="clear-index-btn"
                    onClick={handleClearIndexedData}
                    disabled={isIndexBusy}
                    className="rounded border border-red-500/30 px-3 py-1.5 text-xs font-medium text-red-500 transition-colors hover:bg-red-500/10 disabled:opacity-50"
                  >
                    {clearingIndex ? "Clearing..." : "Clear Index"}
                  </button>
                  <button
                    data-testid="reindex-btn"
                    onClick={handleReindex}
                    disabled={isIndexBusy}
                    className="rounded border border-[var(--color-border)] px-3 py-1.5 text-xs font-medium transition-colors hover:bg-[var(--color-bg-secondary)] hover:text-[var(--color-text)] disabled:opacity-50"
                  >
                    {reindexing ? "Starting..." : "Reindex Now"}
                  </button>
                </div>
                {clearIndexMessage && (
                  <p
                    data-testid="index-action-message"
                    className="mt-3 text-xs text-[var(--color-text-secondary)]"
                  >
                    {clearIndexMessage}
                  </p>
                )}
              </div>
            </div>
          )}

          {tab === "permissions" && <PermissionSettings />}
          {tab === "privacy" && <PrivacyLogViewer />}
        </div>
      </div>
    </div>
  );
}
