import { useSettingsStore, type Theme } from "../../stores/settingsStore";

const THEME_CYCLE: Theme[] = ["system", "light", "dark"];

const THEME_ICON: Record<Theme, string> = {
  system: "\u2699",
  light: "\u2600",
  dark: "\u263D",
};

export function TitleBar() {
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);

  function cycleTheme() {
    const idx = THEME_CYCLE.indexOf(theme);
    setTheme(THEME_CYCLE[(idx + 1) % THEME_CYCLE.length]);
  }

  return (
    <div
      data-testid="title-bar"
      data-tauri-drag-region
      className="flex h-10 items-center justify-end border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)] pr-3 pl-20"
    >
      <button
        aria-label="Toggle theme"
        onClick={cycleTheme}
        className="rounded p-1.5 text-sm hover:bg-[var(--color-border)]"
      >
        {THEME_ICON[theme]}
      </button>
    </div>
  );
}
