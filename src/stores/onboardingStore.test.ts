import { describe, it, expect, vi, beforeEach } from "vitest";
import { useOnboardingStore } from "./onboardingStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../services/settingsService", () => ({
  getSetting: vi.fn().mockResolvedValue(null),
  setSetting: vi.fn().mockResolvedValue(undefined),
}));

describe("onboardingStore", () => {
  beforeEach(() => {
    useOnboardingStore.setState(useOnboardingStore.getInitialState());
  });

  it("initializes with default values", () => {
    const state = useOnboardingStore.getState();
    expect(state.isActive).toBe(false);
    expect(state.currentStep).toBe(0);
    expect(state.completedSteps).toEqual([false, false, false, false]);
  });

  it("starts onboarding", () => {
    useOnboardingStore.getState().start();
    expect(useOnboardingStore.getState().isActive).toBe(true);
    expect(useOnboardingStore.getState().currentStep).toBe(0);
  });

  it("completes a step", () => {
    useOnboardingStore.getState().completeStep(0);
    expect(useOnboardingStore.getState().completedSteps[0]).toBe(true);
  });

  it("does not double-complete same step", () => {
    useOnboardingStore.getState().completeStep(0);
    useOnboardingStore.getState().completeStep(0);
    const steps = useOnboardingStore.getState().completedSteps;
    expect(steps.filter(Boolean).length).toBe(1);
  });

  it("advances to next step", () => {
    useOnboardingStore.getState().nextStep();
    expect(useOnboardingStore.getState().currentStep).toBe(1);
  });

  it("does not exceed step 3", () => {
    useOnboardingStore.setState({ currentStep: 3 });
    useOnboardingStore.getState().nextStep();
    expect(useOnboardingStore.getState().currentStep).toBe(3);
  });

  it("finish sets isActive false and persists", async () => {
    const { setSetting } = await import("../services/settingsService");
    useOnboardingStore.setState({ isActive: true });
    useOnboardingStore.getState().finish();
    expect(useOnboardingStore.getState().isActive).toBe(false);
    expect(setSetting).toHaveBeenCalledWith("onboarding_complete", "true");
  });

  it("checkShouldShow activates when no setting", async () => {
    const shouldShow = await useOnboardingStore.getState().checkShouldShow();
    expect(shouldShow).toBe(true);
    expect(useOnboardingStore.getState().isActive).toBe(true);
    expect(useOnboardingStore.getState().loaded).toBe(true);
  });
});
