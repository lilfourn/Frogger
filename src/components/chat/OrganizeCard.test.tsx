import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { OrganizeCard } from "./OrganizeCard";
import type { OrganizePlan, OrganizePlacement } from "../../utils/actionParser";

const makePlan = (
  categories: Array<{ folder: string; files: string[] }>,
  placements?: OrganizePlacement[],
  stats?: OrganizePlan["stats"],
): OrganizePlan => ({
  categories: categories.map((c) => ({ ...c, description: "" })),
  placements,
  stats,
});

const defaults = {
  folderPath: "/home/user/Downloads",
  phase: "plan-ready" as const,
  executeContent: "",
  error: "",
  onCancelPlan: vi.fn(),
  onApproveAll: vi.fn(),
  onDenyAll: vi.fn(),
  onOpenPath: vi.fn(),
};

describe("OrganizeCard", () => {
  it("renders filenames only, not full paths", () => {
    const plan = makePlan([
      {
        folder: "Documents",
        files: ["/home/user/Downloads/report.pdf", "/home/user/Downloads/notes.txt"],
      },
    ]);
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    // Expand the folder (tree renders segment name, no trailing slash)
    fireEvent.click(screen.getByText("Documents"));

    expect(screen.getByText("report.pdf")).toBeInTheDocument();
    expect(screen.getByText("notes.txt")).toBeInTheDocument();
    expect(screen.queryByText("/home/user/Downloads/report.pdf")).not.toBeInTheDocument();
  });

  it("checkbox toggle adds line-through styling", () => {
    const plan = makePlan([{ folder: "Images", files: ["/pic/a.png", "/pic/b.png"] }]);
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    fireEvent.click(screen.getByText("Images"));
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

    fireEvent.click(screen.getByText("Docs"));
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

    // "Drop" appears in both summary and tree — target the button
    const dropButton = screen.getByRole("button", { name: /Drop/ });
    fireEvent.click(dropButton);
    fireEvent.click(screen.getByLabelText("drop.txt"));
    fireEvent.click(screen.getByText("Approve"));

    const filtered = JSON.parse(onApprovePlan.mock.calls[0][0]);
    expect(filtered.categories).toHaveLength(1);
    expect(filtered.categories[0].folder).toBe("Keep");
  });

  it("folder count reflects exclusions", () => {
    const plan = makePlan([{ folder: "Mixed", files: ["/a.txt", "/b.txt", "/c.txt"] }]);
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    // Summary line shows totals
    expect(screen.getByText(/1 folders · 3\/3 files/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("Mixed"));

    const folderButton = screen.getByText("Mixed").closest("button")!;
    expect(within(folderButton).getByText(/3\/3 file/)).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("b.txt"));

    expect(screen.getByText(/1 folders · 2\/3 files/)).toBeInTheDocument();
    expect(within(folderButton).getByText(/2\/3 file/)).toBeInTheDocument();
  });

  it("filters placements when approving plan", () => {
    const onApprovePlan = vi.fn();
    const plan = makePlan(
      [{ folder: "Documents", files: ["/docs/invoice.pdf", "/docs/contract.pdf"] }],
      [
        {
          path: "/docs/invoice.pdf",
          folder: "documents",
          subfolder: "finance",
          packing_path: "acme_2025_01_pdf_s01",
        },
        {
          path: "/docs/contract.pdf",
          folder: "documents",
          subfolder: "legal",
          packing_path: "legal_2025_01_text",
        },
      ],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={onApprovePlan} />);

    // Tree: documents → finance/acme_2025_01_pdf_s01 (single-child collapsed)
    fireEvent.click(screen.getByText("documents"));
    fireEvent.click(screen.getByText("finance/acme_2025_01_pdf_s01"));
    fireEvent.click(screen.getByLabelText("invoice.pdf"));
    fireEvent.click(screen.getByText("Approve"));

    const filtered = JSON.parse(onApprovePlan.mock.calls[0][0]);
    expect(filtered.placements).toEqual([
      {
        path: "/docs/contract.pdf",
        folder: "documents",
        subfolder: "legal",
        packing_path: "legal_2025_01_text",
      },
    ]);
  });

  it("shows warning when unclassified ratio is high", () => {
    const plan = makePlan([{ folder: "Other", files: ["/a.txt"] }], undefined, {
      total_files: 10,
      indexed_files: 10,
      skipped_hidden: 0,
      skipped_already_organized: 0,
      chunks: 1,
      other_ratio: 0.25,
      max_children_observed: 34,
      folders_over_target: 2,
      packing_llm_calls: 1,
    });

    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);
    expect(screen.getByText("High unclassified ratio: 25.0%")).toBeInTheDocument();
    expect(
      screen.getByText("Packing quality: max children 34, over target 2, LLM refinements 1"),
    ).toBeInTheDocument();
  });

  it("shows missing-source path hint and opens parent location", async () => {
    const onOpenPath = vi.fn().mockResolvedValue(undefined);

    render(
      <OrganizeCard
        {...defaults}
        phase="error"
        plan={null}
        error="Missing source path for move_files action 3/5: /tmp/reports/invoice.pdf. Re-run organization plan and try again."
        onApprovePlan={vi.fn()}
        onOpenPath={onOpenPath}
      />,
    );

    expect(screen.getByText("/tmp/reports/invoice.pdf")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open file location" }));

    await waitFor(() => {
      expect(onOpenPath).toHaveBeenCalledWith("/tmp/reports");
    });
  });

  it("allows cancellation from error state", () => {
    const onDenyAll = vi.fn();

    render(
      <OrganizeCard
        {...defaults}
        phase="error"
        plan={null}
        error="Something failed"
        onApprovePlan={vi.fn()}
        onDenyAll={onDenyAll}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onDenyAll).toHaveBeenCalledOnce();
  });

  it("displays rename arrow for placements with suggested_name", () => {
    const plan = makePlan(
      [{ folder: "vacation_photos", files: ["/pics/IMG_001.jpg"] }],
      [
        {
          path: "/pics/IMG_001.jpg",
          folder: "vacation_photos",
          suggested_name: "beach_sunset.jpg",
        },
      ],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    fireEvent.click(screen.getByText("vacation_photos"));
    expect(screen.getByText("beach_sunset.jpg")).toBeInTheDocument();
    expect(screen.getByText("IMG_001.jpg")).toBeInTheDocument();
  });

  it("shows rename toggle when placements have suggested names", () => {
    const plan = makePlan(
      [{ folder: "photos", files: ["/IMG_001.jpg"] }],
      [
        {
          path: "/IMG_001.jpg",
          folder: "photos",
          suggested_name: "sunset.jpg",
        },
      ],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);
    expect(screen.getByLabelText("Rename files with unclear names")).toBeChecked();
  });

  it("strips suggested_name when rename toggle is off", () => {
    const onApprovePlan = vi.fn();
    const plan = makePlan(
      [{ folder: "photos", files: ["/IMG_001.jpg"] }],
      [
        {
          path: "/IMG_001.jpg",
          folder: "photos",
          suggested_name: "sunset.jpg",
        },
      ],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={onApprovePlan} />);

    fireEvent.click(screen.getByLabelText("Rename files with unclear names"));
    fireEvent.click(screen.getByText("Approve"));

    const filtered = JSON.parse(onApprovePlan.mock.calls[0][0]);
    expect(filtered.placements[0].suggested_name).toBeUndefined();
  });

  it("preserves suggested_name in approved plan when rename toggle is on", () => {
    const onApprovePlan = vi.fn();
    const plan = makePlan(
      [{ folder: "photos", files: ["/IMG_001.jpg"] }],
      [
        {
          path: "/IMG_001.jpg",
          folder: "photos",
          suggested_name: "sunset.jpg",
        },
      ],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={onApprovePlan} />);
    fireEvent.click(screen.getByText("Approve"));

    const filtered = JSON.parse(onApprovePlan.mock.calls[0][0]);
    expect(filtered.placements[0].suggested_name).toBe("sunset.jpg");
  });

  it("works with freeform folder names", () => {
    const plan = makePlan(
      [{ folder: "tax_returns", files: ["/invoice.pdf"] }],
      [{ path: "/invoice.pdf", folder: "tax_returns", subfolder: "2024" }],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);
    // Single-child collapsing: tax_returns/2024 rendered as combined name
    expect(screen.getByText("tax_returns/2024")).toBeInTheDocument();
  });

  it("summary shows totals and top-level breakdown when 2+ roots", () => {
    const plan = makePlan(
      [],
      [
        { path: "/a.pdf", folder: "documents", subfolder: "finance" },
        { path: "/b.pdf", folder: "documents", subfolder: "finance" },
        { path: "/c.jpg", folder: "photos" },
      ],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    expect(screen.getByText(/2 folders/)).toBeInTheDocument();
    expect(screen.getByText(/3\/3 files/)).toBeInTheDocument();
    // Names appear in both summary breakdown and tree nodes
    expect(screen.getAllByText("documents").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("photos").length).toBeGreaterThanOrEqual(1);
  });

  it("rename badge shows count on collapsed folder", () => {
    const plan = makePlan(
      [{ folder: "photos", files: ["/a.jpg", "/b.jpg"] }],
      [
        { path: "/a.jpg", folder: "photos", suggested_name: "sunset.jpg" },
        { path: "/b.jpg", folder: "photos", suggested_name: "beach.jpg" },
      ],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    // Folder is collapsed — rename badge should be visible
    expect(screen.getByText("2 renamed")).toBeInTheDocument();
  });

  it("rename badge hidden when folder expanded", () => {
    const plan = makePlan(
      [{ folder: "photos", files: ["/a.jpg"] }],
      [{ path: "/a.jpg", folder: "photos", suggested_name: "sunset.jpg" }],
    );
    render(<OrganizeCard {...defaults} plan={plan} onApprovePlan={vi.fn()} />);

    expect(screen.getByText("1 renamed")).toBeInTheDocument();

    // Expand folder — badge should disappear, individual rename indicators shown
    fireEvent.click(screen.getByText("photos"));
    expect(screen.queryByText("1 renamed")).not.toBeInTheDocument();
    expect(screen.getByText("sunset.jpg")).toBeInTheDocument();
  });
});
