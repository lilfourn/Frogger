import { describe, it, expect } from "vitest";
import type { OrganizePlan } from "./actionParser";
import {
  createEditablePlan,
  toggleFile,
  editFileName,
  editFolderLabel,
  setRenameEnabled,
  countActiveFiles,
  countTotalFiles,
  countRenames,
  hasAnyRenames,
  toApprovedPlanJson,
  buildFolderTree,
} from "./editablePlan";

const makePlan = (
  categories: Array<{ folder: string; files: string[] }>,
  placements?: OrganizePlan["placements"],
  extra?: Partial<OrganizePlan>,
): OrganizePlan => ({
  categories: categories.map((c) => ({ ...c, description: "" })),
  placements,
  ...extra,
});

describe("createEditablePlan", () => {
  it("groups placements into folders with correct names", () => {
    const plan = makePlan(
      [{ folder: "docs", files: ["/a.pdf", "/b.pdf"] }],
      [
        { path: "/a.pdf", folder: "docs", subfolder: "finance", suggested_name: "invoice.pdf" },
        { path: "/b.pdf", folder: "docs", subfolder: "legal" },
      ],
    );
    const ep = createEditablePlan(plan);

    expect(ep.folders).toHaveLength(2);
    expect(ep.folders[0].label).toBe("docs/finance/");
    expect(ep.folders[0].originalLabel).toBe("docs/finance");
    expect(ep.folders[0].files[0].suggestedName).toBe("invoice.pdf");
    expect(ep.folders[0].files[0].originalName).toBe("a.pdf");
    expect(ep.folders[0].files[0].included).toBe(true);

    expect(ep.folders[1].label).toBe("docs/legal/");
    expect(ep.folders[1].files[0].suggestedName).toBeUndefined();
  });

  it("handles packing_path in placements", () => {
    const plan = makePlan([], [
      { path: "/x.jpg", folder: "photos", subfolder: "2024", packing_path: "vacation_batch_01" },
    ]);
    const ep = createEditablePlan(plan);

    expect(ep.folders[0].id).toBe("photos/2024/vacation_batch_01");
    expect(ep.folders[0].label).toBe("photos/2024/vacation_batch_01/");
  });

  it("creates folders from categories when no placements", () => {
    const plan = makePlan([
      { folder: "Images", files: ["/pic/a.png", "/pic/b.png"] },
      { folder: "Docs", files: ["/doc/c.pdf"] },
    ]);
    const ep = createEditablePlan(plan);

    expect(ep.folders).toHaveLength(2);
    expect(ep.folders[0].id).toBe("Images-0");
    expect(ep.folders[0].label).toBe("Images/");
    expect(ep.folders[0].originalLabel).toBe("Images");
    expect(ep.folders[0].files).toHaveLength(2);
    expect(ep.folders[0].files[0].originalName).toBe("a.png");
    expect(ep.folders[1].files[0].originalName).toBe("c.pdf");
  });

  it("defaults renameEnabled to true", () => {
    const ep = createEditablePlan(makePlan([{ folder: "A", files: ["/x"] }]));
    expect(ep.renameEnabled).toBe(true);
  });

  it("preserves stats from source plan", () => {
    const stats = { total_files: 5, indexed_files: 5, skipped_hidden: 0, skipped_already_organized: 0, chunks: 1 };
    const ep = createEditablePlan(makePlan([{ folder: "A", files: ["/x"] }], undefined, { stats }));
    expect(ep.stats).toEqual(stats);
  });

  it("uses 'other' as default folder for placements without folder", () => {
    const plan = makePlan([], [{ path: "/x.txt", folder: "" }]);
    const ep = createEditablePlan(plan);
    expect(ep.folders[0].label).toBe("other/");
  });
});

describe("toggleFile", () => {
  it("flips included flag on matching file", () => {
    const ep = createEditablePlan(makePlan([{ folder: "A", files: ["/a", "/b"] }]));
    const toggled = toggleFile(ep, "/a");

    expect(toggled.folders[0].files[0].included).toBe(false);
    expect(toggled.folders[0].files[1].included).toBe(true);
  });

  it("toggles back to included", () => {
    const ep = createEditablePlan(makePlan([{ folder: "A", files: ["/a"] }]));
    const result = toggleFile(toggleFile(ep, "/a"), "/a");
    expect(result.folders[0].files[0].included).toBe(true);
  });
});

describe("editFileName", () => {
  it("sets editedName on correct file", () => {
    const ep = createEditablePlan(makePlan([{ folder: "A", files: ["/a.txt", "/b.txt"] }]));
    const edited = editFileName(ep, "/a.txt", "renamed.txt");

    expect(edited.folders[0].files[0].editedName).toBe("renamed.txt");
    expect(edited.folders[0].files[1].editedName).toBeUndefined();
  });
});

describe("editFolderLabel", () => {
  it("updates folder label by id", () => {
    const ep = createEditablePlan(makePlan([{ folder: "Old", files: ["/x"] }]));
    const edited = editFolderLabel(ep, "Old-0", "New/");

    expect(edited.folders[0].label).toBe("New/");
    expect(edited.folders[0].originalLabel).toBe("Old");
  });

  it("does not change other folders", () => {
    const ep = createEditablePlan(
      makePlan([
        { folder: "A", files: ["/x"] },
        { folder: "B", files: ["/y"] },
      ]),
    );
    const edited = editFolderLabel(ep, "A-0", "C/");
    expect(edited.folders[1].label).toBe("B/");
  });
});

describe("setRenameEnabled", () => {
  it("toggles renameEnabled flag", () => {
    const ep = createEditablePlan(makePlan([{ folder: "A", files: ["/x"] }]));
    expect(setRenameEnabled(ep, false).renameEnabled).toBe(false);
    expect(setRenameEnabled(ep, true).renameEnabled).toBe(true);
  });
});

describe("countActiveFiles / countTotalFiles", () => {
  it("counts all files when none excluded", () => {
    const ep = createEditablePlan(
      makePlan([
        { folder: "A", files: ["/a", "/b"] },
        { folder: "B", files: ["/c"] },
      ]),
    );
    expect(countTotalFiles(ep)).toBe(3);
    expect(countActiveFiles(ep)).toBe(3);
  });

  it("reflects exclusions in active count", () => {
    let ep = createEditablePlan(makePlan([{ folder: "A", files: ["/a", "/b", "/c"] }]));
    ep = toggleFile(ep, "/b");

    expect(countTotalFiles(ep)).toBe(3);
    expect(countActiveFiles(ep)).toBe(2);
  });
});

describe("countRenames", () => {
  it("counts files with suggested or edited names", () => {
    const plan = makePlan(
      [{ folder: "photos", files: ["/a.jpg", "/b.jpg", "/c.jpg"] }],
      [
        { path: "/a.jpg", folder: "photos", suggested_name: "sunset.jpg" },
        { path: "/b.jpg", folder: "photos" },
        { path: "/c.jpg", folder: "photos", suggested_name: "beach.jpg" },
      ],
    );
    const ep = createEditablePlan(plan);
    expect(countRenames(ep)).toBe(2);
  });

  it("returns 0 when renameEnabled is false", () => {
    const plan = makePlan(
      [{ folder: "photos", files: ["/a.jpg"] }],
      [{ path: "/a.jpg", folder: "photos", suggested_name: "sunset.jpg" }],
    );
    const ep = setRenameEnabled(createEditablePlan(plan), false);
    expect(countRenames(ep)).toBe(0);
  });

  it("excludes files that are not included", () => {
    const plan = makePlan(
      [{ folder: "photos", files: ["/a.jpg", "/b.jpg"] }],
      [
        { path: "/a.jpg", folder: "photos", suggested_name: "sunset.jpg" },
        { path: "/b.jpg", folder: "photos", suggested_name: "beach.jpg" },
      ],
    );
    let ep = createEditablePlan(plan);
    ep = toggleFile(ep, "/a.jpg");
    expect(countRenames(ep)).toBe(1);
  });

  it("counts user-edited names", () => {
    const plan = makePlan(
      [{ folder: "docs", files: ["/a.txt"] }],
      [{ path: "/a.txt", folder: "docs" }],
    );
    let ep = createEditablePlan(plan);
    ep = editFileName(ep, "/a.txt", "readme.txt");
    expect(countRenames(ep)).toBe(1);
  });
});

describe("hasAnyRenames", () => {
  it("returns true when any file has suggestedName", () => {
    const plan = makePlan(
      [{ folder: "p", files: ["/a.jpg"] }],
      [{ path: "/a.jpg", folder: "p", suggested_name: "s.jpg" }],
    );
    expect(hasAnyRenames(createEditablePlan(plan))).toBe(true);
  });

  it("returns false when no files have suggestedName", () => {
    const ep = createEditablePlan(makePlan([{ folder: "A", files: ["/a"] }]));
    expect(hasAnyRenames(ep)).toBe(false);
  });
});

describe("buildFolderTree", () => {
  it("single-level folders become flat root nodes", () => {
    const ep = createEditablePlan(
      makePlan([
        { folder: "Images", files: ["/a.png"] },
        { folder: "Docs", files: ["/b.pdf"] },
      ]),
    );
    const tree = buildFolderTree(ep);

    expect(tree).toHaveLength(2);
    expect(tree[0].name).toBe("Docs");
    expect(tree[1].name).toBe("Images");
    expect(tree[0].folder).not.toBeNull();
    expect(tree[0].children).toHaveLength(0);
  });

  it("multi-level paths nest correctly (3 levels deep)", () => {
    const plan = makePlan(
      [],
      [
        { path: "/a.pdf", folder: "docs", subfolder: "finance" },
      ],
    );
    const ep = createEditablePlan(plan);
    const tree = buildFolderTree(ep);

    expect(tree).toHaveLength(1);
    expect(tree[0].name).toBe("docs");
    expect(tree[0].folder).toBeNull();
    expect(tree[0].children).toHaveLength(1);
    expect(tree[0].children[0].name).toBe("finance");
    expect(tree[0].children[0].folder).not.toBeNull();
  });

  it("intermediate nodes have folder: null", () => {
    const plan = makePlan(
      [],
      [{ path: "/x.jpg", folder: "photos", subfolder: "2024", packing_path: "vacation" }],
    );
    const ep = createEditablePlan(plan);
    const tree = buildFolderTree(ep);

    expect(tree[0].name).toBe("photos");
    expect(tree[0].folder).toBeNull();
    expect(tree[0].children[0].name).toBe("2024");
    expect(tree[0].children[0].folder).toBeNull();
    expect(tree[0].children[0].children[0].name).toBe("vacation");
    expect(tree[0].children[0].children[0].folder).not.toBeNull();
  });

  it("node with both own files and children works", () => {
    const plan = makePlan(
      [],
      [
        { path: "/readme.md", folder: "docs" },
        { path: "/invoice.pdf", folder: "docs", subfolder: "finance" },
      ],
    );
    const ep = createEditablePlan(plan);
    const tree = buildFolderTree(ep);

    const docsNode = tree[0];
    expect(docsNode.name).toBe("docs");
    expect(docsNode.folder).not.toBeNull();
    expect(docsNode.folder!.files).toHaveLength(1);
    expect(docsNode.children).toHaveLength(1);
    expect(docsNode.children[0].name).toBe("finance");
  });

  it("aggregated stats sum correctly through tree", () => {
    const plan = makePlan(
      [],
      [
        { path: "/a.pdf", folder: "docs", subfolder: "finance", suggested_name: "invoice.pdf" },
        { path: "/b.pdf", folder: "docs", subfolder: "finance" },
        { path: "/c.pdf", folder: "docs", subfolder: "legal" },
      ],
    );
    const ep = createEditablePlan(plan);
    const tree = buildFolderTree(ep);

    const docsNode = tree[0];
    expect(docsNode.fileCount).toBe(3);
    expect(docsNode.activeFileCount).toBe(3);
    expect(docsNode.renameCount).toBe(1);

    const financeNode = docsNode.children[0];
    expect(financeNode.fileCount).toBe(2);
    expect(financeNode.renameCount).toBe(1);
  });

  it("empty plan returns empty array", () => {
    const ep = createEditablePlan(makePlan([]));
    expect(buildFolderTree(ep)).toEqual([]);
  });

  it("folders with shared prefix but different paths don't merge", () => {
    const plan = makePlan(
      [],
      [
        { path: "/a.txt", folder: "documents", subfolder: "tax" },
        { path: "/b.txt", folder: "documentation" },
      ],
    );
    const ep = createEditablePlan(plan);
    const tree = buildFolderTree(ep);

    expect(tree).toHaveLength(2);
    expect(tree[0].name).toBe("documentation");
    expect(tree[1].name).toBe("documents");
    expect(tree[1].children[0].name).toBe("tax");
  });
});

describe("toApprovedPlanJson", () => {
  it("excludes files that are not included", () => {
    const source = makePlan([{ folder: "Docs", files: ["/a.txt", "/b.txt", "/c.txt"] }]);
    let ep = createEditablePlan(source);
    ep = toggleFile(ep, "/b.txt");

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.categories[0].files).toEqual(["/a.txt", "/c.txt"]);
  });

  it("drops empty categories after exclusion", () => {
    const source = makePlan([
      { folder: "Keep", files: ["/keep.txt"] },
      { folder: "Drop", files: ["/drop.txt"] },
    ]);
    let ep = createEditablePlan(source);
    ep = toggleFile(ep, "/drop.txt");

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.categories).toHaveLength(1);
    expect(result.categories[0].folder).toBe("Keep");
  });

  it("applies name edits to placements when renameEnabled", () => {
    const source = makePlan(
      [{ folder: "photos", files: ["/a.jpg"] }],
      [{ path: "/a.jpg", folder: "photos", suggested_name: "old.jpg" }],
    );
    let ep = createEditablePlan(source);
    ep = editFileName(ep, "/a.jpg", "new.jpg");

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.placements[0].suggested_name).toBe("new.jpg");
  });

  it("strips suggested_name when renameEnabled is false", () => {
    const source = makePlan(
      [{ folder: "photos", files: ["/a.jpg"] }],
      [{ path: "/a.jpg", folder: "photos", suggested_name: "sunset.jpg" }],
    );
    let ep = createEditablePlan(source);
    ep = setRenameEnabled(ep, false);

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.placements[0].suggested_name).toBeUndefined();
  });

  it("preserves suggested_name when renameEnabled and no edit", () => {
    const source = makePlan(
      [{ folder: "photos", files: ["/a.jpg"] }],
      [{ path: "/a.jpg", folder: "photos", suggested_name: "sunset.jpg" }],
    );
    const ep = createEditablePlan(source);

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.placements[0].suggested_name).toBe("sunset.jpg");
  });

  it("applies folder label edits to categories", () => {
    const source = makePlan([{ folder: "Old", files: ["/x.txt"] }]);
    let ep = createEditablePlan(source);
    ep = editFolderLabel(ep, "Old-0", "New/");

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.categories[0].folder).toBe("New");
  });

  it("applies folder label edits to placements", () => {
    const source = makePlan(
      [{ folder: "photos", files: ["/a.jpg"] }],
      [{ path: "/a.jpg", folder: "photos" }],
    );
    let ep = createEditablePlan(source);
    ep = editFolderLabel(ep, "photos", "pictures/");

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.placements[0].folder).toBe("pictures");
  });

  it("filters excluded placements", () => {
    const source = makePlan(
      [{ folder: "docs", files: ["/a.pdf", "/b.pdf"] }],
      [
        { path: "/a.pdf", folder: "docs" },
        { path: "/b.pdf", folder: "docs" },
      ],
    );
    let ep = createEditablePlan(source);
    ep = toggleFile(ep, "/a.pdf");

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.placements).toHaveLength(1);
    expect(result.placements[0].path).toBe("/b.pdf");
  });

  it("preserves stats and taxonomy_version from source", () => {
    const stats = { total_files: 5, indexed_files: 5, skipped_hidden: 0, skipped_already_organized: 0, chunks: 1 };
    const source = makePlan([{ folder: "A", files: ["/x"] }], undefined, {
      stats,
      taxonomy_version: "v2",
    });
    const ep = createEditablePlan(source);

    const result = JSON.parse(toApprovedPlanJson(ep, source));
    expect(result.stats).toEqual(stats);
    expect(result.taxonomy_version).toBe("v2");
  });
});
