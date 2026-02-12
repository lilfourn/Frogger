import { useState, useEffect, useRef } from "react";
import { Settings, X } from "lucide-react";
import { saveApiKey, hasApiKey, deleteApiKey } from "../../services/settingsService";
import { PermissionSettings } from "./PermissionSettings";
import { PrivacyLogViewer } from "./PrivacyLogViewer";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
  const [tab, setTab] = useState<"general" | "permissions" | "privacy">("general");
  const [keyInput, setKeyInput] = useState("");
  const [keyExists, setKeyExists] = useState(false);
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      hasApiKey()
        .then(setKeyExists)
        .catch((err) => console.error("[Settings] Failed to check API key:", err));
      setKeyInput("");
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  async function handleSave() {
    if (!keyInput.trim()) return;
    setSaving(true);
    try {
      await saveApiKey(keyInput.trim());
      setKeyExists(true);
      setKeyInput("");
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete() {
    await deleteApiKey();
    setKeyExists(false);
  }

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
            className="rounded px-2 py-0.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
          >
            <X size={14} strokeWidth={1.5} />
          </button>
        </div>

        <div className="flex gap-1 border-b border-[var(--color-border)] px-4">
          {(["general", "permissions", "privacy"] as const).map((t) => (
            <button
              key={t}
              data-testid={`tab-${t}`}
              onClick={() => setTab(t)}
              className={`px-2 py-1.5 text-xs capitalize ${tab === t ? "border-b-2 border-[var(--color-accent)] text-[var(--color-text)]" : "text-[var(--color-text-secondary)]"}`}
            >
              {t}
            </button>
          ))}
        </div>

        <div className="space-y-4 px-4 py-3">
          {tab === "general" && (
            <div>
              <div className="mb-2 flex items-center gap-2">
                <h3 className="text-xs font-semibold uppercase text-[var(--color-text-secondary)]">
                  API Key
                </h3>
                <span
                  data-testid="key-status"
                  className={`h-2 w-2 rounded-full ${keyExists ? "bg-[var(--color-accent)]" : "bg-[var(--color-border)]"}`}
                />
              </div>

              <div className="flex items-center gap-2">
                <input
                  ref={inputRef}
                  data-testid="api-key-input"
                  type="password"
                  value={keyInput}
                  onChange={(e) => setKeyInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSave();
                  }}
                  placeholder={keyExists ? "Key saved — enter new key to replace" : "sk-ant-..."}
                  className="w-full rounded border border-[var(--color-border)] bg-transparent px-3 py-1.5 text-sm outline-none placeholder:text-[var(--color-text-secondary)]"
                />
              </div>

              <div className="mt-2 flex items-center gap-2">
                <button
                  data-testid="save-key-btn"
                  onClick={handleSave}
                  disabled={!keyInput.trim() || saving}
                  className="rounded bg-[var(--color-accent)] px-3 py-1 text-sm text-white disabled:opacity-50"
                >
                  Save
                </button>
                {keyExists && (
                  <button
                    data-testid="delete-key-btn"
                    onClick={handleDelete}
                    className="rounded px-3 py-1 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
                  >
                    Remove
                  </button>
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
