import { useEffect } from "react";
import { listDirectory } from "./services/fileService";
import { useFileStore } from "./stores/fileStore";
import { AppLayout } from "./components/layout/AppLayout";
import { Sidebar } from "./components/sidebar/Sidebar";
import { Toolbar } from "./components/toolbar/Toolbar";
import { FileView } from "./components/file-view/FileView";
import { useTheme } from "./hooks/useTheme";

function App() {
  useTheme();

  const currentPath = useFileStore((s) => s.currentPath);
  const entries = useFileStore((s) => s.entries);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const setEntries = useFileStore((s) => s.setEntries);
  const setError = useFileStore((s) => s.setError);
  const setLoading = useFileStore((s) => s.setLoading);

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
