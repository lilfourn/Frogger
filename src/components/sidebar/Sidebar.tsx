import { useEffect, useState } from "react";
import { useFileStore } from "../../stores/fileStore";
import { getHomeDir, getMountedVolumes } from "../../services/fileService";
import type { VolumeInfo } from "../../types/volume";

interface Bookmark {
  name: string;
  path: string;
}

function buildBookmarks(homeDir: string): Bookmark[] {
  return [
    { name: "Home", path: homeDir },
    { name: "Desktop", path: `${homeDir}/Desktop` },
    { name: "Documents", path: `${homeDir}/Documents` },
    { name: "Downloads", path: `${homeDir}/Downloads` },
  ];
}

export function Sidebar() {
  const navigateTo = useFileStore((s) => s.navigateTo);
  const currentPath = useFileStore((s) => s.currentPath);
  const recentPaths = useFileStore((s) => s.recentPaths);

  const [bookmarks, setBookmarks] = useState<Bookmark[]>(() =>
    buildBookmarks("/Users"),
  );
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
              className={`w-full rounded px-2 py-1 text-left hover:bg-[var(--color-border)] ${
                currentPath === b.path ? "bg-[var(--color-accent)] text-white" : ""
              }`}
            >
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
                className={`w-full rounded px-2 py-1 text-left hover:bg-[var(--color-border)] ${
                  currentPath === v.path ? "bg-[var(--color-accent)] text-white" : ""
                }`}
              >
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
                className={`w-full truncate rounded px-2 py-1 text-left hover:bg-[var(--color-border)] ${
                  currentPath === p ? "bg-[var(--color-accent)] text-white" : ""
                }`}
              >
                {p}
              </button>
            ))}
          </div>
        </section>
      )}
    </nav>
  );
}
