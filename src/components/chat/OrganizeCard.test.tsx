import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import { OrganizeCard } from "./OrganizeCard";
import type { OrganizePlan } from "../../utils/actionParser";

const makePlan = (
  categories: Array<{ folder: string; files: string[] }>,
): OrganizePlan => ({
  categories: categories.map((c) => ({ ...c, description: "" })),
});

const defaults = {
  folderPath: "/home/user/Downloads",
  phase: "plan-ready" as const,
  executeContent: "",
  error: "",
  onCancelPlan: vi.fn(),
  onApproveAll: vi.fn(),
  onDenyAll: vi.fn(),
};

describe("OrganizeCard", () => {
  it("renders filenames only, not full paths", () => {
    const plan = makePlan([
      { folder: "Documents", files: ["/home/user/Downloads/report.pdf", "/home/user/Downloads/notes.txt"] },
    ]);
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    // Expand the folder
    fireEvent.click(screen.getByText("Documents/"));

    expect(screen.getByText("report.pdf")).toBeInTheDocument();
    expect(screen.getByText("notes.txt")).toBeInTheDocument();
    expect(screen.queryByText("/home/user/Downloads/report.pdf")).not.toBeInTheDocument();
  });

  it("checkbox toggle adds line-through styling", () => {
    const plan = makePlan([
      { folder: "Images", files: ["/pic/a.png", "/pic/b.png"] },
    ]);
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    fireEvent.click(screen.getByText("Images/"));
    const checkbox = screen.getByLabelText("a.png");
    const label = checkbox.closest("label")!;

    expect(label.className).not.toContain("line-through");

    fireEvent.click(checkbox);
    expect(label.className).toContain("line-through");

    fireEvent.click(checkbox);
    expect(label.className).not.toContain("line-through");
  });

  it("approve sends filtered plan JSON without excluded files", () => {
    const onApprovePlan = vi.fn();
    const plan = makePlan([
      { folder: "Docs", files: ["/a/one.txt", "/a/two.txt", "/a/three.txt"] },
    ]);
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={onApprovePlan} />);

    fireEvent.click(screen.getByText("Docs/"));
    fireEvent.click(screen.getByLabelText("two.txt"));
    fireEvent.click(screen.getByText("Approve"));

    expect(onApprovePlan).toHaveBeenCalledOnce();
    const filtered = JSON.parse(onApprovePlan.mock.calls[0][0]);
    expect(filtered.categories[0].files).toEqual(["/a/one.txt", "/a/three.txt"]);
  });

  it("excluding all files in a category drops it from output", () => {
    const onApprovePlan = vi.fn();
    const plan = makePlan([
      { folder: "Keep", files: ["/keep.txt"] },
      { folder: "Drop", files: ["/drop.txt"] },
    ]);
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={onApprovePlan} />);

    fireEvent.click(screen.getByText("Drop/"));
    fireEvent.click(screen.getByLabelText("drop.txt"));
    fireEvent.click(screen.getByText("Approve"));

    const filtered = JSON.parse(onApprovePlan.mock.calls[0][0]);
    expect(filtered.categories).toHaveLength(1);
    expect(filtered.categories[0].folder).toBe("Keep");
  });

  it("folder count reflects exclusions", () => {
    const plan = makePlan([
      { folder: "Mixed", files: ["/a.txt", "/b.txt", "/c.txt"] },
    ]);
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    expect(screen.getByText(/3\/3 files in Downloads/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("Mixed/"));

    const folderButton = screen.getByText("Mixed/").closest("button")!;
    expect(within(folderButton).getByText(/3\/3 file/)).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("b.txt"));

    expect(screen.getByText(/2\/3 files in Downloads/)).toBeInTheDocument();
    expect(within(folderButton).getByText(/2\/3 file/)).toBeInTheDocument();
  });
});
