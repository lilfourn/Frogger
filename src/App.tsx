import { useEffect, useMemo } from "react";
import { listDirectory } from "./services/fileService";
import { useFileStore } from "./stores/fileStore";
import { AppLayout } from "./components/layout/AppLayout";
import { Sidebar } from "./components/sidebar/Sidebar";
import { Toolbar } from "./components/toolbar/Toolbar";
import { FileView } from "./components/file-view/FileView";

import { TabBar } from "./components/tabs/TabBar";
import { QuickLookPanel } from "./components/quick-look/QuickLookPanel";
import { SearchBar } from "./components/search/SearchBar";
import { useTheme } from "./hooks/useTheme";
import { useFileOperations } from "./hooks/useFileOperations";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { useQuickLook } from "./hooks/useQuickLook";
import { useSettingsStore } from "./stores/settingsStore";
import { useSearchStore } from "./stores/searchStore";

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
    ],
  );

  useKeyboardShortcuts(shortcuts);

  useEffect(() => {
    const defaultPath = navigator.userAgent.includes("Windows") ? "C:\\Users" : "/Users";
    if (!currentPath) navigateTo(defaultPath);
  }, [currentPath, navigateTo]);

  useEffect(() => {
    if (!currentPath) return;
    let cancelled = false;
    setLoading(true);
    listDirectory(currentPath)
      .then((result) => {
        if (!cancelled) {
          setEntries(result);
          setLoading(false);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [currentPath, setEntries, setError, setLoading]);

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
        itemCount={entries.length}
        currentPath={currentPath}
      />
      <QuickLookPanel
        isOpen={quickLook.isOpen}
        filePath={quickLook.filePath}
        previewType={quickLook.previewType}
        onClose={quickLook.close}
      />
      <SearchBar />
    </>
  );
}

export default App;
