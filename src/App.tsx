import { useEffect, useMemo } from "react";
import { listDirectory } from "./services/fileService";
import { useFileStore } from "./stores/fileStore";
import { AppLayout } from "./components/layout/AppLayout";
import { Sidebar } from "./components/sidebar/Sidebar";
import { Toolbar } from "./components/toolbar/Toolbar";
import { FileView } from "./components/file-view/FileView";
import { Breadcrumb } from "./components/file-view/Breadcrumb";
import { useTheme } from "./hooks/useTheme";
import { useFileOperations } from "./hooks/useFileOperations";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";

function App() {
  useTheme();

  const currentPath = useFileStore((s) => s.currentPath);
  const entries = useFileStore((s) => s.entries);
  const selectedFiles = useFileStore((s) => s.selectedFiles);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const goUp = useFileStore((s) => s.goUp);
  const setEntries = useFileStore((s) => s.setEntries);
  const setError = useFileStore((s) => s.setError);
  const setLoading = useFileStore((s) => s.setLoading);

  const { undo, redo, deleteFiles, rename, createDir } = useFileOperations();

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
      { key: "Backspace", handler: goUp },
    ],
    [undo, redo, deleteFiles, selectedFiles, rename, createDir, goUp],
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
      <Breadcrumb />
      <Toolbar />
      <FileView />
    </div>
  );

  return (
    <AppLayout
      sidebar={<Sidebar />}
      main={main}
      itemCount={entries.length}
      currentPath={currentPath}
    />
  );
}

export default App;
