import { create } from "zustand";
import { getSetting, setSetting } from "../services/settingsService";

interface OnboardingState {
  isActive: boolean;
  currentStep: number;
  completedSteps: boolean[];
  loaded: boolean;
  start: () => void;
  completeStep: (index: number) => void;
  nextStep: () => void;
  finish: () => void;
  checkShouldShow: () => Promise<boolean>;
}

export const useOnboardingStore = create<OnboardingState>()((set) => ({
  isActive: false,
  currentStep: 0,
  completedSteps: [false, false, false, false],
  loaded: false,

  start: () => set({ isActive: true, currentStep: 0 }),

  completeStep: (index: number) =>
    set((s) => {
      if (s.completedSteps[index]) return s;
      const completedSteps = [...s.completedSteps];
      completedSteps[index] = true;
      return { completedSteps };
    }),

  nextStep: () =>
    set((s) => ({
      currentStep: Math.min(s.currentStep + 1, 3),
    })),

  finish: () => {
    set({ isActive: false });
    setSetting("onboarding_complete", "true").catch((err) => console.error("[Onboarding] Failed to save completion:", err));
  },

  checkShouldShow: async () => {
    const val = await getSetting("onboarding_complete");
    const shouldShow = val !== "true";
    set({ loaded: true, isActive: shouldShow });
    return shouldShow;
  },
}));
