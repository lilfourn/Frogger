export interface FileAction {
  tool:
    | "move_files"
    | "copy_files"
    | "rename_file"
    | "delete_files"
    | "create_directory";
  args: Record<string, unknown>;
}

export interface OrganizePlanStats {
  total_files: number;
  indexed_files: number;
  skipped_hidden: number;
  skipped_already_organized: number;
  chunks: number;
}

export interface OrganizePlan {
  taxonomy_version?: string;
  categories: Array<{
    folder: string;
    description: string;
    files: string[];
  }>;
  unclassified?: string[];
  stats?: OrganizePlanStats;
}

export interface ParsedSegment {
  type: "text" | "action";
  content: string;
  action?: FileAction;
}

const ACTION_BLOCK_RE = /```action\n([\s\S]*?)```/g;

const VALID_TOOLS = new Set([
  "move_files",
  "copy_files",
  "rename_file",
  "delete_files",
  "create_directory",
]);

function hasStringArray(obj: Record<string, unknown>, key: string): boolean {
  return Array.isArray(obj[key]) && obj[key].every((v: unknown) => typeof v === "string");
}

function hasString(obj: Record<string, unknown>, key: string): boolean {
  return typeof obj[key] === "string";
}

export function validateActionArgs(tool: string, args: Record<string, unknown>): boolean {
  switch (tool) {
    case "move_files":
    case "copy_files":
      return hasStringArray(args, "sources") && hasString(args, "dest_dir");
    case "rename_file":
      return hasString(args, "source") && hasString(args, "destination");
    case "delete_files":
      return hasStringArray(args, "paths");
    case "create_directory":
      return hasString(args, "path");
    default:
      return false;
  }
}

export function parseActionBlocks(text: string): ParsedSegment[] {
  const segments: ParsedSegment[] = [];
  let lastIndex = 0;

  for (const match of text.matchAll(ACTION_BLOCK_RE)) {
    const before = text.slice(lastIndex, match.index);
    if (before.trim()) {
      segments.push({ type: "text", content: before });
    }

    try {
      const parsed = JSON.parse(match[1].trim()) as FileAction;
      if (VALID_TOOLS.has(parsed.tool) && validateActionArgs(parsed.tool, parsed.args ?? {})) {
        segments.push({ type: "action", content: match[0], action: parsed });
      } else {
        segments.push({ type: "text", content: match[0] });
      }
    } catch {
      segments.push({ type: "text", content: match[0] });
    }

    lastIndex = match.index! + match[0].length;
  }

  const remaining = text.slice(lastIndex);
  if (remaining.trim()) {
    segments.push({ type: "text", content: remaining });
  }

  return segments;
}

export function parseOrganizePlan(text: string): OrganizePlan | null {
  const jsonMatch = text.match(/```(?:json|plan)?\n([\s\S]*?)```/);
  const raw = jsonMatch ? jsonMatch[1].trim() : text.trim();
  try {
    const parsed = JSON.parse(raw);
    if (parsed.categories && Array.isArray(parsed.categories)) {
      return parsed as OrganizePlan;
    }
  } catch {
    /* not valid JSON */
  }
  return null;
}

export function describeAction(action: FileAction): string {
  const { tool, args } = action;
  switch (tool) {
    case "move_files": {
      const sources = args.sources as string[];
      const destName = (args.dest_dir as string).split("/").pop();
      return `Move ${sources.length} file${sources.length > 1 ? "s" : ""} → ${destName}/`;
    }
    case "copy_files": {
      const sources = args.sources as string[];
      const destName = (args.dest_dir as string).split("/").pop();
      return `Copy ${sources.length} file${sources.length > 1 ? "s" : ""} → ${destName}/`;
    }
    case "rename_file":
      return `Rename → ${(args.destination as string).split("/").pop()}`;
    case "delete_files": {
      const paths = args.paths as string[];
      return `Delete ${paths.length} file${paths.length > 1 ? "s" : ""}`;
    }
    case "create_directory":
      return `Create ${(args.path as string).split("/").pop()}/`;
    default:
      return `Unknown action: ${tool}`;
  }
}

export function getActionFilePaths(action: FileAction): string[] {
  const { tool, args } = action;
  switch (tool) {
    case "move_files":
    case "copy_files":
      return args.sources as string[];
    case "rename_file":
      return [args.source as string];
    case "delete_files":
      return args.paths as string[];
    case "create_directory":
      return [args.path as string];
    default:
      return [];
  }
}
