import { describe, it, expect } from "vitest";
import {
  parseActionBlocks,
  parseOrganizePlan,
  describeAction,
  getActionFilePaths,
} from "./actionParser";
import type { FileAction } from "./actionParser";

describe("parseActionBlocks", () => {
  it("returns plain text when no action blocks", () => {
    const result = parseActionBlocks("Just some text");
    expect(result).toEqual([{ type: "text", content: "Just some text" }]);
  });

  it("parses a single action block", () => {
    const input =
      'Here is what I\'ll do:\n```action\n{"tool": "move_files", "args": {"sources": ["/a.txt"], "dest_dir": "/b"}}\n```';
    const result = parseActionBlocks(input);
    expect(result).toHaveLength(2);
    expect(result[0].type).toBe("text");
    expect(result[1].type).toBe("action");
    expect(result[1].action?.tool).toBe("move_files");
  });

  it("parses multiple action blocks with text between", () => {
    const input =
      'First:\n```action\n{"tool": "create_directory", "args": {"path": "/new"}}\n```\nThen:\n```action\n{"tool": "move_files", "args": {"sources": ["/a"], "dest_dir": "/new"}}\n```';
    const result = parseActionBlocks(input);
    expect(result).toHaveLength(4);
    expect(result[0].type).toBe("text");
    expect(result[1].type).toBe("action");
    expect(result[2].type).toBe("text");
    expect(result[3].type).toBe("action");
  });

  it("treats invalid JSON as text", () => {
    const input = "```action\nnot json\n```";
    const result = parseActionBlocks(input);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("text");
  });

  it("treats unknown tools as text", () => {
    const input = '```action\n{"tool": "hack_system", "args": {}}\n```';
    const result = parseActionBlocks(input);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("text");
  });

  it("handles action block with no surrounding text", () => {
    const input = '```action\n{"tool": "delete_files", "args": {"paths": ["/trash.txt"]}}\n```';
    const result = parseActionBlocks(input);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("action");
    expect(result[0].action?.tool).toBe("delete_files");
  });
});

describe("describeAction", () => {
  it("describes move_files", () => {
    const action: FileAction = {
      tool: "move_files",
      args: { sources: ["/a.txt", "/b.txt"], dest_dir: "/docs" },
    };
    expect(describeAction(action)).toBe("Move 2 files → docs/");
  });

  it("describes single file move", () => {
    const action: FileAction = {
      tool: "move_files",
      args: { sources: ["/a.txt"], dest_dir: "/docs" },
    };
    expect(describeAction(action)).toBe("Move 1 file → docs/");
  });

  it("describes copy_files", () => {
    const action: FileAction = {
      tool: "copy_files",
      args: { sources: ["/a.txt"], dest_dir: "/backup" },
    };
    expect(describeAction(action)).toBe("Copy 1 file → backup/");
  });

  it("describes rename_file", () => {
    const action: FileAction = {
      tool: "rename_file",
      args: { source: "/old.txt", destination: "/new.txt" },
    };
    expect(describeAction(action)).toBe("Rename → new.txt");
  });

  it("describes delete_files", () => {
    const action: FileAction = {
      tool: "delete_files",
      args: { paths: ["/a.txt", "/b.txt", "/c.txt"] },
    };
    expect(describeAction(action)).toBe("Delete 3 files");
  });

  it("describes create_directory", () => {
    const action: FileAction = {
      tool: "create_directory",
      args: { path: "/Users/test/NewFolder" },
    };
    expect(describeAction(action)).toBe("Create NewFolder/");
  });
});

describe("parseOrganizePlan", () => {
  it("parses plan from ```json block", () => {
    const input =
      '```json\n{"categories": [{"folder": "images", "description": "Image files", "files": ["a.png", "b.jpg"]}]}\n```';
    const plan = parseOrganizePlan(input);
    expect(plan).not.toBeNull();
    expect(plan!.categories).toHaveLength(1);
    expect(plan!.categories[0].folder).toBe("images");
    expect(plan!.categories[0].files).toEqual(["a.png", "b.jpg"]);
  });

  it("parses plan from raw JSON", () => {
    const input =
      '{"categories": [{"folder": "docs", "description": "Documents", "files": ["readme.md"]}]}';
    const plan = parseOrganizePlan(input);
    expect(plan).not.toBeNull();
    expect(plan!.categories[0].folder).toBe("docs");
  });

  it("returns null for invalid JSON", () => {
    expect(parseOrganizePlan("not json at all")).toBeNull();
  });

  it("returns null for JSON without categories", () => {
    expect(parseOrganizePlan('{"files": []}')).toBeNull();
  });

  it("parses plan from ```plan block", () => {
    const input =
      '```plan\n{"categories": [{"folder": "other", "description": "Misc", "files": ["x.txt"]}]}\n```';
    const plan = parseOrganizePlan(input);
    expect(plan).not.toBeNull();
    expect(plan!.categories[0].folder).toBe("other");
  });
});

describe("getActionFilePaths", () => {
  it("returns sources for move_files", () => {
    const action: FileAction = {
      tool: "move_files",
      args: { sources: ["/a", "/b"], dest_dir: "/c" },
    };
    expect(getActionFilePaths(action)).toEqual(["/a", "/b"]);
  });

  it("returns source for rename_file", () => {
    const action: FileAction = {
      tool: "rename_file",
      args: { source: "/old", destination: "/new" },
    };
    expect(getActionFilePaths(action)).toEqual(["/old"]);
  });

  it("returns paths for delete_files", () => {
    const action: FileAction = {
      tool: "delete_files",
      args: { paths: ["/x"] },
    };
    expect(getActionFilePaths(action)).toEqual(["/x"]);
  });

  it("returns path for create_directory", () => {
    const action: FileAction = {
      tool: "create_directory",
      args: { path: "/new" },
    };
    expect(getActionFilePaths(action)).toEqual(["/new"]);
  });
});
