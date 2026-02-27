import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { OrganizeModal } from "./OrganizeModal";
import { useChatStore } from "../../stores/chatStore";

const mocks = vi.hoisted(() => ({
  cancelActiveOrganize: vi.fn(),
}));

vi.mock("./OrganizeCard", () => ({
  OrganizeCard: (props: { onCancelPlan: () => void; onDenyAll: () => void }) => (
    <div data-testid="organize-card">
      <button data-testid="organize-cancel-plan" onClick={props.onCancelPlan}>
        Cancel Plan
      </button>
      <button data-testid="organize-deny-all" onClick={props.onDenyAll}>
        Cancel Review
      </button>
    </div>
  ),
}));

vi.mock("../../hooks/useChat", () => ({
  useChat: () => ({
    executeOrganize: vi.fn(),
    applyOrganize: vi.fn(),
    cancelActiveOrganize: mocks.cancelActiveOrganize,
    retryOrganize: vi.fn(),
    openOrganizePath: vi.fn(),
  }),
}));

describe("OrganizeModal", () => {
  beforeEach(() => {
    useChatStore.setState(useChatStore.getInitialState());
    mocks.cancelActiveOrganize.mockReset();
  });

  it("shows phase percent label while keeping overall bar monotonic", () => {
    useChatStore.setState({
      organize: {
        phase: "planning",
        folderPath: "/tmp/project",
        plan: null,
        planRaw: "",
        executeContent: "",
        error: "",
        progress: {
          sessionId: "organize-1",
          rootPath: "/tmp/project",
          phase: "indexing",
          processed: 10,
          total: 30,
          percent: 33,
          combinedPercent: 8,
          message: "Indexing files 10/30",
          sequence: 5,
        },
      },
    });

    render(<OrganizeModal />);

    expect(screen.getByText("Indexing files... 33%")).toBeInTheDocument();
    expect(screen.getByTestId("organize-progress-bar")).toHaveStyle({ width: "8%" });
    expect(screen.getByText("Indexing files 10/30")).toBeInTheDocument();
  });

  it("centers the loading progress shell when no card content is shown", () => {
    useChatStore.setState({
      organize: {
        phase: "planning",
        folderPath: "/tmp/project",
        plan: null,
        planRaw: "",
        executeContent: "",
        error: "",
        progress: {
          sessionId: "organize-2",
          rootPath: "/tmp/project",
          phase: "planning",
          processed: 2,
          total: 10,
          percent: 20,
          combinedPercent: 12,
          message: "Planning",
          sequence: 6,
        },
      },
    });

    render(<OrganizeModal />);

    const progressShell = screen.getByTestId("organize-progress-shell");
    const modalCard = progressShell.parentElement as HTMLElement;

    expect(modalCard).toHaveClass("flex", "min-h-[220px]", "flex-col", "justify-center");
    expect(progressShell).not.toHaveClass("mb-4");
  });

  it("does not close when clicking the backdrop", () => {
    useChatStore.setState({
      organize: {
        phase: "planning",
        folderPath: "/tmp/project",
        plan: null,
        planRaw: "",
        executeContent: "",
        error: "",
        progress: null,
      },
    });

    render(<OrganizeModal />);

    fireEvent.click(screen.getByTestId("organize-modal"));

    expect(screen.getByTestId("organize-modal")).toBeInTheDocument();
    expect(useChatStore.getState().organize.phase).toBe("planning");
  });

  it("does not close on Escape", () => {
    useChatStore.setState({
      organize: {
        phase: "planning",
        folderPath: "/tmp/project",
        plan: null,
        planRaw: "",
        executeContent: "",
        error: "",
        progress: null,
      },
    });

    render(<OrganizeModal />);

    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.getByTestId("organize-modal")).toBeInTheDocument();
    expect(useChatStore.getState().organize.phase).toBe("planning");
  });

  it("routes cancel actions through cancelActiveOrganize", () => {
    useChatStore.setState({
      organize: {
        phase: "planning",
        folderPath: "/tmp/project",
        plan: null,
        planRaw: "",
        executeContent: "",
        error: "",
        progress: null,
      },
    });

    render(<OrganizeModal />);

    fireEvent.click(screen.getByTestId("organize-cancel-plan"));
    fireEvent.click(screen.getByTestId("organize-deny-all"));

    expect(mocks.cancelActiveOrganize).toHaveBeenCalledTimes(2);
  });
});
