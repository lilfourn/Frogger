import { useEffect, useState } from "react";
import { Home, Monitor, FileText, Download, HardDrive, Clock, type LucideIcon } from "lucide-react";
import { useFileStore } from "../../stores/fileStore";
import { getHomeDir, getMountedVolumes } from "../../services/fileService";
import type { VolumeInfo } from "../../types/volume";

interface Bookmark {
  name: string;
  path: string;
  icon: LucideIcon;
}

function buildBookmarks(homeDir: string): Bookmark[] {
  return [
    { name: "Home", path: homeDir, icon: Home },
    { name: "Desktop", path: `${homeDir}/Desktop`, icon: Monitor },
    { name: "Documents", path: `${homeDir}/Documents`, icon: FileText },
    { name: "Downloads", path: `${homeDir}/Downloads`, icon: Download },
  ];
}

export function Sidebar() {
  const navigateTo = useFileStore((s) => s.navigateTo);
  const recentPaths = useFileStore((s) => s.recentPaths);

  const [bookmarks, setBookmarks] = useState<Bookmark[]>(() => buildBookmarks("/Users"));
  const [volumes, setVolumes] = useState<VolumeInfo[]>([]);

  useEffect(() => {
    getHomeDir()
      .then((home) => setBookmarks(buildBookmarks(home)))
      .catch(() => {});
    getMountedVolumes()
      .then(setVolumes)
      .catch(() => {});
  }, []);

  return (
    <nav className="flex h-full flex-col gap-4 p-3 text-sm">
      <section>
        <h3 className="mb-1 text-xs font-semibold uppercase text-[var(--color-text-secondary)]">
          Favorites
        </h3>
        <div className="space-y-0.5">
          {bookmarks.map((b) => (
            <button
              key={b.name}
              onClick={() => navigateTo(b.path)}
              className="flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-[var(--color-border)]"
            >
              <b.icon size={15} strokeWidth={1.5} className="shrink-0" />
              {b.name}
            </button>
          ))}
        </div>
      </section>

      {volumes.length > 0 && (
        <section>
          <h3 className="mb-1 text-xs font-semibold uppercase text-[var(--color-text-secondary)]">
            Volumes
          </h3>
          <div className="space-y-0.5">
            {volumes.map((v) => (
              <button
                key={v.path}
                onClick={() => navigateTo(v.path)}
                className="flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-[var(--color-border)]"
              >
                <HardDrive size={15} strokeWidth={1.5} className="shrink-0" />
                {v.name}
              </button>
            ))}
          </div>
        </section>
      )}

      {recentPaths.length > 0 && (
        <section>
          <h3 className="mb-1 text-xs font-semibold uppercase text-[var(--color-text-secondary)]">
            Recents
          </h3>
          <div className="space-y-0.5">
            {recentPaths.slice(0, 10).map((p) => (
              <button
                key={p}
                onClick={() => navigateTo(p)}
                className="flex w-full items-center gap-2 truncate rounded px-2 py-1 text-left hover:bg-[var(--color-border)]"
              >
                <Clock size={15} strokeWidth={1.5} className="shrink-0" />
                <span className="truncate">{p}</span>
              </button>
            ))}
          </div>
        </section>
      )}
    </nav>
  );
}
