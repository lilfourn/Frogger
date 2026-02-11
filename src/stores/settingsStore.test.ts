import { describe, it, expect, beforeEach } from "vitest";
import { useSettingsStore } from "./settingsStore";

describe("settingsStore", () => {
  beforeEach(() => {
    useSettingsStore.setState(useSettingsStore.getInitialState());
  });

  it("has correct defaults", () => {
    const state = useSettingsStore.getState();
    expect(state.theme).toBe("system");
    expect(state.viewMode).toBe("list");
    expect(state.sidebarWidth).toBe(240);
    expect(state.sidebarVisible).toBe(true);
  });

  it("setTheme updates theme", () => {
    useSettingsStore.getState().setTheme("dark");
    expect(useSettingsStore.getState().theme).toBe("dark");
  });

  it("setViewMode updates viewMode", () => {
    useSettingsStore.getState().setViewMode("grid");
    expect(useSettingsStore.getState().viewMode).toBe("grid");
  });

  it("setSidebarWidth updates width", () => {
    useSettingsStore.getState().setSidebarWidth(300);
    expect(useSettingsStore.getState().sidebarWidth).toBe(300);
  });

  it("toggleSidebar flips visibility", () => {
    useSettingsStore.getState().toggleSidebar();
    expect(useSettingsStore.getState().sidebarVisible).toBe(false);
    useSettingsStore.getState().toggleSidebar();
    expect(useSettingsStore.getState().sidebarVisible).toBe(true);
  });

  it("resolvedTheme returns light or dark based on system preference", () => {
    useSettingsStore.getState().setTheme("light");
    expect(useSettingsStore.getState().resolvedTheme()).toBe("light");

    useSettingsStore.getState().setTheme("dark");
    expect(useSettingsStore.getState().resolvedTheme()).toBe("dark");
  });
});
