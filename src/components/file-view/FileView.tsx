import { useSettingsStore } from "../../stores/settingsStore";
import { useFileStore } from "../../stores/fileStore";
import { ListView } from "./ListView";
import { GridView } from "./GridView";

export function FileView() {
  const viewMode = useSettingsStore((s) => s.viewMode);
  const sortedEntries = useFileStore((s) => s.sortedEntries);
  const navigateTo = useFileStore((s) => s.navigateTo);
  const error = useFileStore((s) => s.error);

  const entries = sortedEntries();

  function handleNavigate(entry: { is_directory: boolean; path: string }) {
    if (entry.is_directory) navigateTo(entry.path);
  }

  return (
    <div className="h-full overflow-auto">
      {error && <div className="p-4 text-red-500">{error}</div>}
      {viewMode === "grid" ? (
        <GridView entries={entries} onNavigate={handleNavigate} />
      ) : (
        <ListView entries={entries} onNavigate={handleNavigate} />
      )}
    </div>
  );
}
