import { useEffect } from "react";
import { useSettingsStore } from "../stores/settingsStore";

export function useTheme() {
  const theme = useSettingsStore((s) => s.theme);
  const resolvedTheme = useSettingsStore((s) => s.resolvedTheme);

  useEffect(() => {
    function apply() {
      const resolved = resolvedTheme();
      document.documentElement.setAttribute("data-theme", resolved);
    }

    apply();

    if (theme === "system") {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      mq.addEventListener("change", apply);
      return () => mq.removeEventListener("change", apply);
    }
  }, [theme, resolvedTheme]);
}
