import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ToolConfirmation } from "./ToolConfirmation";
import type { FileAction } from "../../utils/actionParser";

describe("ToolConfirmation", () => {
  const moveAction: FileAction = {
    tool: "move_files",
    args: { sources: ["/Users/test/a.txt", "/Users/test/b.pdf"], dest_dir: "/Users/test/docs" },
  };

  it("renders action description", () => {
    render(<ToolConfirmation action={moveAction} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByText("Move 2 files → docs/")).toBeInTheDocument();
  });

  it("renders file names", () => {
    render(<ToolConfirmation action={moveAction} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByText("a.txt")).toBeInTheDocument();
    expect(screen.getByText("b.pdf")).toBeInTheDocument();
  });

  it("shows approve and deny buttons", () => {
    render(<ToolConfirmation action={moveAction} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByTestId("tool-approve")).toBeInTheDocument();
    expect(screen.getByTestId("tool-deny")).toBeInTheDocument();
  });

  it("calls onApprove and shows done state", async () => {
    const onApprove = vi.fn().mockResolvedValue(undefined);
    render(<ToolConfirmation action={moveAction} onApprove={onApprove} onDeny={vi.fn()} />);
    fireEvent.click(screen.getByTestId("tool-approve"));
    await waitFor(() => expect(screen.getByText("Done")).toBeInTheDocument());
    expect(onApprove).toHaveBeenCalledWith(moveAction);
  });

  it("calls onDeny and shows denied state", () => {
    const onDeny = vi.fn();
    render(<ToolConfirmation action={moveAction} onApprove={vi.fn()} onDeny={onDeny} />);
    fireEvent.click(screen.getByTestId("tool-deny"));
    expect(screen.getByText("Denied")).toBeInTheDocument();
    expect(onDeny).toHaveBeenCalledWith(moveAction);
  });

  it("shows running state during execution", async () => {
    let resolve: () => void;
    const onApprove = vi.fn(
      () =>
        new Promise<void>((r) => {
          resolve = r;
        }),
    );
    render(<ToolConfirmation action={moveAction} onApprove={onApprove} onDeny={vi.fn()} />);
    fireEvent.click(screen.getByTestId("tool-approve"));
    expect(screen.getByText("Running...")).toBeInTheDocument();
    resolve!();
    await waitFor(() => expect(screen.getByText("Done")).toBeInTheDocument());
  });

  it("renders delete action description", () => {
    const deleteAction: FileAction = {
      tool: "delete_files",
      args: { paths: ["/trash.txt"] },
    };
    render(<ToolConfirmation action={deleteAction} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByText("Delete 1 file")).toBeInTheDocument();
  });

  it("renders create_directory action", () => {
    const createAction: FileAction = {
      tool: "create_directory",
      args: { path: "/Users/test/Reports" },
    };
    render(<ToolConfirmation action={createAction} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByText("Create Reports/")).toBeInTheDocument();
  });
});
