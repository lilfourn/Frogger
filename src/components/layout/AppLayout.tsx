import type { ReactNode } from "react";
import { useSettingsStore } from "../../stores/settingsStore";
import { TitleBar } from "./TitleBar";
import { StatusBar } from "./StatusBar";

interface AppLayoutProps {
  sidebar: ReactNode;
  main: ReactNode;
  itemCount?: number;
  currentPath?: string;
}

export function AppLayout({
  sidebar,
  main,
  itemCount = 0,
  currentPath,
}: AppLayoutProps) {
  const sidebarVisible = useSettingsStore((s) => s.sidebarVisible);
  const sidebarWidth = useSettingsStore((s) => s.sidebarWidth);

  return (
    <div className="flex h-screen flex-col bg-[var(--color-bg)]">
      <TitleBar />

      <div className="flex flex-1 overflow-hidden">
        {sidebarVisible && (
          <aside
            data-testid="sidebar"
            style={{ width: sidebarWidth }}
            className="flex-shrink-0 overflow-y-auto border-r border-[var(--color-border)] bg-[var(--color-sidebar-bg)]"
          >
            {sidebar}
          </aside>
        )}

        <main data-testid="main-panel" className="flex-1 overflow-auto">
          {main}
        </main>
      </div>

      <StatusBar itemCount={itemCount} currentPath={currentPath} />
    </div>
  );
}
