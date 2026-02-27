import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { OnboardingModal } from "./OnboardingModal";
import { useOnboardingStore } from "../../stores/onboardingStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../../services/settingsService", () => ({
  saveApiKey: vi.fn().mockResolvedValue(undefined),
  hasApiKey: vi.fn().mockResolvedValue(false),
  deleteApiKey: vi.fn().mockResolvedValue(undefined),
  getSetting: vi.fn().mockResolvedValue(null),
  setSetting: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../services/chatService", () => ({
  sendChat: vi.fn().mockResolvedValue("response"),
  getChatHistory: vi.fn().mockResolvedValue([]),
  clearChatHistory: vi.fn().mockResolvedValue(undefined),
  newChatSession: vi.fn().mockResolvedValue("test-session"),
  sendOrganizePlan: vi.fn().mockResolvedValue(""),
  sendOrganizeExecute: vi.fn().mockResolvedValue(""),
  sendOrganizeApply: vi.fn().mockResolvedValue(""),
  cancelOrganize: vi.fn().mockResolvedValue(undefined),
  getOrganizeStatus: vi.fn().mockResolvedValue(null),
}));

vi.mock("../../hooks/useChat", () => ({
  useChat: () => ({
    send: vi.fn(),
    startOrganize: vi.fn(),
    executeOrganize: vi.fn(),
    applyOrganize: vi.fn(),
    cancelActiveOrganize: vi.fn(),
    resetSession: vi.fn(),
  }),
}));

vi.mock("../../stores/fileStore", () => ({
  useFileStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      currentPath: "/Users/test",
      selectedFiles: [],
    }),
}));

describe("OnboardingModal", () => {
  beforeEach(() => {
    useOnboardingStore.setState(useOnboardingStore.getInitialState());
  });

  it("renders nothing when not active", () => {
    const { container } = render(<OnboardingModal />);
    expect(container.querySelector("[data-testid='onboarding-modal']")).toBeNull();
  });

  it("renders full-screen modal when active", () => {
    useOnboardingStore.setState({ isActive: true });
    render(<OnboardingModal />);
    const modal = screen.getByTestId("onboarding-modal");
    expect(modal).toBeInTheDocument();
    expect(modal.className).toContain("fixed inset-0");
  });

  it("shows welcome step initially", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 0 });
    render(<OnboardingModal />);
    expect(screen.getByText("Welcome to Frogger")).toBeInTheDocument();
    expect(screen.getByText("Let's Go")).toBeInTheDocument();
  });

  it("advances from welcome to API key step", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 0 });
    render(<OnboardingModal />);
    fireEvent.click(screen.getByTestId("onboarding-next"));
    expect(useOnboardingStore.getState().currentStep).toBe(1);
  });

  it("shows API key step", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 1 });
    render(<OnboardingModal />);
    expect(screen.getByText("Connect Your Brain")).toBeInTheDocument();
    expect(screen.getByTestId("onboarding-api-input")).toBeInTheDocument();
  });

  it("shows chat step", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 2 });
    render(<OnboardingModal />);
    expect(screen.getByText("Say Hello")).toBeInTheDocument();
  });

  it("shows suggestion chips on chat step", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 2 });
    render(<OnboardingModal />);
    const suggestions = screen.getAllByTestId("onboarding-suggestion");
    expect(suggestions.length).toBe(2);
  });

  it("shows complete step", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 3 });
    render(<OnboardingModal />);
    expect(screen.getByText("You're Ready!")).toBeInTheDocument();
    expect(screen.getByText("Cmd+Shift+C to open chat anytime")).toBeInTheDocument();
  });

  it("finishes onboarding from complete step", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 3 });
    render(<OnboardingModal />);
    fireEvent.click(screen.getByTestId("onboarding-finish"));
    expect(useOnboardingStore.getState().isActive).toBe(false);
  });

  it("skip button finishes onboarding", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 0 });
    render(<OnboardingModal />);
    fireEvent.click(screen.getByTestId("onboarding-skip"));
    expect(useOnboardingStore.getState().isActive).toBe(false);
  });

  it("shows feature pills on welcome step", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 0 });
    render(<OnboardingModal />);
    expect(screen.getByText("Smart Search")).toBeInTheDocument();
    expect(screen.getByText("AI Chat")).toBeInTheDocument();
    expect(screen.getByText("File Actions")).toBeInTheDocument();
  });

  it("does not show skip on complete step", () => {
    useOnboardingStore.setState({ isActive: true, currentStep: 3 });
    render(<OnboardingModal />);
    expect(screen.queryByTestId("onboarding-skip")).toBeNull();
  });
});
