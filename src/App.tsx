import { useEffect, useMemo, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getHomeDir, listDirectory } from "./services/fileService";
import { useFileStore } from "./stores/fileStore";
import { AppLayout } from "./components/layout/AppLayout";
import { Sidebar } from "./components/sidebar/Sidebar";
import { Toolbar } from "./components/toolbar/Toolbar";
import { FileView } from "./components/file-view/FileView";

import { TabBar } from "./components/tabs/TabBar";
import { QuickLookPanel } from "./components/quick-look/QuickLookPanel";
import { SearchBar } from "./components/search/SearchBar";
import { SettingsModal } from "./components/settings/SettingsModal";
import { ChatPanel } from "./components/chat/ChatPanel";
import { OnboardingModal } from "./components/onboarding/OnboardingModal";
import { OrganizeModal } from "./components/chat/OrganizeModal";
import { PermissionPromptModal } from "./components/permissions/PermissionPromptModal";
import { useTheme } from "./hooks/useTheme";
import { useFileOperations } from "./hooks/useFileOperations";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { useQuickLook } from "./hooks/useQuickLook";
import { getSetting, normalizePermissionScopes, setSetting } from "./services/settingsService";
import { useSettingsStore } from "./stores/settingsStore";
import { useSearchStore } from "./stores/searchStore";
import { useChatStore } from "./stores/chatStore";
import { useOnboardingStore } from "./stores/onboardingStore";

const PERMISSION_SCOPE_NORMALIZATION_STATUS_KEY = "permission_scope_normalization_status_v1";
const PERMISSION_SCOPE_NORMALIZATION_ATTEMPTS_KEY = "permission_scope_normalization_attempts_v1";
const PERMISSION_SCOPE_NORMALIZATION_NEXT_RUN_AT_KEY =
  "permission_scope_normalization_next_run_at_v1";
const PERMISSION_SCOPE_NORMALIZATION_MAX_BACKOFF_MS = 4 * 60 * 60 * 1000;
const PERMISSION_SCOPE_NORMALIZATION_START_DELAY_MS = 2000;
const INDEXING_START_RETRY_DELAYS_MS = [0, 1000, 2500];

function computeNormalizationBackoffMs(attempt: number): number {
  const base = 30000;
  const exponential = base * 2 ** Math.max(attempt - 1, 0);
  return Math.min(exponential, PERMISSION_SCOPE_NORMALIZATION_MAX_BACKOFF_MS);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function App() {
  useTheme();

  const currentPath = useFileStore((s) => s.currentPath);
  const entries = useFileStore((s) => s.entries);
  const selectedFiles = useFileStore((s) => s.selectedFiles);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const goUp = useFileStore((s) => s.goUp);
  const addTab = useFileStore((s) => s.addTab);
  const closeTab = useFileStore((s) => s.closeTab);
  const activeTabId = useFileStore((s) => s.activeTabId);
  const setEntries = useFileStore((s) => s.setEntries);
  const setError = useFileStore((s) => s.setError);
  const setLoading = useFileStore((s) => s.setLoading);

  const toggleHiddenFiles = useSettingsStore((s) => s.toggleHiddenFiles);
  const openSearch = useSearchStore((s) => s.open);
  const { undo, redo, deleteFiles, rename, createDir } = useFileOperations();
  const quickLook = useQuickLook();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const toggleSettings = useCallback(() => setSettingsOpen((o) => !o), []);
  const toggleChat = useChatStore((s) => s.toggle);
  const checkOnboarding = useOnboardingStore((s) => s.checkShouldShow);
  const indexingStarted = useRef(false);
  const indexingStartPending = useRef(false);

  const shortcuts = useMemo(
    () => [
      { key: "z", meta: true, handler: undo },
      { key: "z", meta: true, shift: true, handler: redo },
      {
        key: "Backspace",
        meta: true,
        handler: () => {
          if (selectedFiles.length > 0) deleteFiles(selectedFiles);
        },
      },
      {
        key: "F2",
        handler: () => {
          if (selectedFiles.length === 1) {
            const newName = prompt("New name:");
            if (newName) rename(selectedFiles[0], newName);
          }
        },
      },
      {
        key: "n",
        meta: true,
        shift: true,
        handler: () => {
          const name = prompt("Folder name:");
          if (name) createDir(name);
        },
      },
      { key: "t", meta: true, handler: addTab },
      { key: "w", meta: true, handler: () => closeTab(activeTabId) },
      {
        key: " ",
        handler: () => {
          if (selectedFiles.length === 1) quickLook.toggle(selectedFiles[0]);
        },
      },
      { key: "Backspace", handler: goUp },
      { key: ".", meta: true, shift: true, handler: toggleHiddenFiles },
      { key: "f", meta: true, handler: openSearch },
      { key: "p", meta: true, handler: openSearch },
      { key: ",", meta: true, handler: toggleSettings },
      { key: "c", meta: true, shift: true, handler: toggleChat },
    ],
    [
      undo,
      redo,
      deleteFiles,
      selectedFiles,
      rename,
      createDir,
      addTab,
      closeTab,
      activeTabId,
      quickLook,
      goUp,
      toggleHiddenFiles,
      openSearch,
      toggleSettings,
      toggleChat,
    ],
  );

  useKeyboardShortcuts(shortcuts);

  useEffect(() => {
    let cancelled = false;
    if (currentPath) return () => {};

    console.debug("[App] Resolving home directory...");
    getHomeDir()
      .then((homePath) => {
        console.debug("[App] Home dir resolved:", homePath);
        if (!cancelled && homePath) {
          navigateTo(homePath);
        }
      })
      .catch((err) => {
        console.error("[App] Failed to get home dir, using fallback:", err);
        const fallbackPath = navigator.userAgent.includes("Windows") ? "C:\\Users" : "/Users";
        if (!cancelled) {
          navigateTo(fallbackPath);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [currentPath, navigateTo]);

  const startBackgroundIndexing = useCallback(async (): Promise<boolean> => {
    const homePath = await getHomeDir();
    if (!homePath) return false;

    let lastError: unknown = null;
    for (const delayMs of INDEXING_START_RETRY_DELAYS_MS) {
      if (delayMs > 0) {
        await sleep(delayMs);
      }
      try {
        await invoke("start_indexing", { directory: homePath });
        return true;
      } catch (err) {
        lastError = err;
      }
    }

    console.error("[App] Background indexing failed after retries:", lastError);
    return false;
  }, []);

  useEffect(() => {
    let cancelled = false;

    const schedulePermissionScopeNormalization = async () => {
      console.debug("[App] Checking permission scope normalization status...");
      const [statusRaw, attemptsRaw, nextRunRaw] = await Promise.all([
        getSetting(PERMISSION_SCOPE_NORMALIZATION_STATUS_KEY).catch(() => null),
        getSetting(PERMISSION_SCOPE_NORMALIZATION_ATTEMPTS_KEY).catch(() => null),
        getSetting(PERMISSION_SCOPE_NORMALIZATION_NEXT_RUN_AT_KEY).catch(() => null),
      ]);
      console.debug("[App] Normalization status:", statusRaw);

      if (cancelled || statusRaw === "success") {
        return;
      }

      const now = Date.now();
      const nextRunAt = Number.parseInt(nextRunRaw ?? "0", 10);
      if (Number.isFinite(nextRunAt) && nextRunAt > now) {
        return;
      }

      await setSetting(PERMISSION_SCOPE_NORMALIZATION_STATUS_KEY, "in_progress");

      try {
        console.debug("[App] Running normalizePermissionScopes...");
        await normalizePermissionScopes();
        console.debug("[App] normalizePermissionScopes completed");
        if (cancelled) {
          return;
        }
        await Promise.all([
          setSetting(PERMISSION_SCOPE_NORMALIZATION_STATUS_KEY, "success"),
          setSetting(PERMISSION_SCOPE_NORMALIZATION_ATTEMPTS_KEY, "0"),
          setSetting(PERMISSION_SCOPE_NORMALIZATION_NEXT_RUN_AT_KEY, "0"),
        ]);
      } catch (err) {
        console.error("[App] Permission scope normalization failed:", err);
        const attempts = Number.parseInt(attemptsRaw ?? "0", 10);
        const nextAttempts = Number.isFinite(attempts) ? attempts + 1 : 1;
        const backoffMs = computeNormalizationBackoffMs(nextAttempts);
        if (cancelled) {
          return;
        }
        await Promise.all([
          setSetting(PERMISSION_SCOPE_NORMALIZATION_STATUS_KEY, "failed"),
          setSetting(PERMISSION_SCOPE_NORMALIZATION_ATTEMPTS_KEY, String(nextAttempts)),
          setSetting(
            PERMISSION_SCOPE_NORMALIZATION_NEXT_RUN_AT_KEY,
            String(Date.now() + backoffMs),
          ),
        ]);
      }
    };

    const normalizationTimer = window.setTimeout(() => {
      if (cancelled) return;
      schedulePermissionScopeNormalization().catch((err) =>
        console.error("[App] Permission normalization failed:", err),
      );
    }, PERMISSION_SCOPE_NORMALIZATION_START_DELAY_MS);
    checkOnboarding().catch((err) => console.error("[App] Onboarding check failed:", err));

    return () => {
      cancelled = true;
      window.clearTimeout(normalizationTimer);
    };
  }, [checkOnboarding]);

  useEffect(() => {
    if (!currentPath) return;
    let cancelled = false;
    void invoke("notify_user_interaction").catch((err) =>
      console.debug("[App] Failed to notify user interaction:", err),
    );
    setLoading(true);
    console.debug("[App] Listing directory:", currentPath);
    listDirectory(currentPath)
      .then((result) => {
        console.debug("[App] Directory listed, entries:", result.length);
        if (!cancelled) {
          setEntries(result);
          setLoading(false);
          if (!indexingStarted.current && !indexingStartPending.current) {
            indexingStartPending.current = true;
            console.debug("[App] Triggering deferred background indexing");
            startBackgroundIndexing()
              .then((started) => {
                if (started) {
                  indexingStarted.current = true;
                }
              })
              .finally(() => {
                indexingStartPending.current = false;
              });
          }
        }
      })
      .catch((e) => {
        console.error("[App] Directory listing failed:", e);
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [currentPath, setEntries, setError, setLoading, startBackgroundIndexing]);

  const main = (
    <div className="flex h-full flex-col">
      <TabBar />
      <Toolbar />
      <FileView />
    </div>
  );

  return (
    <>
      <AppLayout
        sidebar={<Sidebar />}
        main={main}
        rightPanel={<ChatPanel />}
        itemCount={entries.length}
        currentPath={currentPath}
        onSettingsClick={toggleSettings}
      />
      <QuickLookPanel
        isOpen={quickLook.isOpen}
        filePath={quickLook.filePath}
        previewType={quickLook.previewType}
        onClose={quickLook.close}
      />
      <SearchBar />
      <SettingsModal isOpen={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <OrganizeModal />
      <OnboardingModal />
      <PermissionPromptModal />
    </>
  );
}

export default App;
