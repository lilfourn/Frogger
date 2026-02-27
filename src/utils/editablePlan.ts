import type { OrganizePlan, OrganizePlanStats } from "./actionParser";

export interface EditableFile {
  path: string;
  originalName: string;
  suggestedName?: string;
  editedName?: string;
  included: boolean;
}

export interface EditableFolder {
  id: string;
  label: string;
  originalLabel: string;
  files: EditableFile[];
}

export interface EditablePlan {
  folders: EditableFolder[];
  renameEnabled: boolean;
  stats?: OrganizePlanStats;
}

export function createEditablePlan(plan: OrganizePlan): EditablePlan {
  let folders: EditableFolder[];

  if (plan.placements && plan.placements.length > 0) {
    const groups = new Map<string, Map<string, string | undefined>>();
    for (const placement of plan.placements) {
      const baseFolder = placement.folder || "other";
      const base = placement.subfolder
        ? `${baseFolder}/${placement.subfolder}`
        : baseFolder;
      const label = placement.packing_path
        ? `${base}/${placement.packing_path}`
        : base;
      const fileMap =
        groups.get(label) ?? new Map<string, string | undefined>();
      fileMap.set(placement.path, placement.suggested_name);
      groups.set(label, fileMap);
    }
    folders = [...groups.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([label, fileMap]) => ({
        id: label,
        label: `${label}/`,
        originalLabel: label,
        files: [...fileMap.entries()]
          .sort(([a], [b]) => a.localeCompare(b))
          .map(([path, suggestedName]) => ({
            path,
            originalName: path.split("/").pop() || path,
            suggestedName: suggestedName ?? undefined,
            included: true,
          })),
      }));
  } else {
    folders = plan.categories.map((category, index) => ({
      id: `${category.folder}-${index}`,
      label: `${category.folder}/`,
      originalLabel: category.folder,
      files: [...category.files].map((path) => ({
        path,
        originalName: path.split("/").pop() || path,
        included: true,
      })),
    }));
  }

  return { folders, renameEnabled: true, stats: plan.stats };
}

// Pure operations — all return new EditablePlan

export function toggleFile(plan: EditablePlan, path: string): EditablePlan {
  return {
    ...plan,
    folders: plan.folders.map((folder) => ({
      ...folder,
      files: folder.files.map((f) =>
        f.path === path ? { ...f, included: !f.included } : f,
      ),
    })),
  };
}

export function editFileName(
  plan: EditablePlan,
  path: string,
  newName: string,
): EditablePlan {
  return {
    ...plan,
    folders: plan.folders.map((folder) => ({
      ...folder,
      files: folder.files.map((f) =>
        f.path === path ? { ...f, editedName: newName } : f,
      ),
    })),
  };
}

export function editFolderLabel(
  plan: EditablePlan,
  folderId: string,
  newLabel: string,
): EditablePlan {
  return {
    ...plan,
    folders: plan.folders.map((folder) =>
      folder.id === folderId ? { ...folder, label: newLabel } : folder,
    ),
  };
}

export function setRenameEnabled(
  plan: EditablePlan,
  enabled: boolean,
): EditablePlan {
  return { ...plan, renameEnabled: enabled };
}

// Derived values

export function countActiveFiles(plan: EditablePlan): number {
  return plan.folders.reduce(
    (sum, folder) => sum + folder.files.filter((f) => f.included).length,
    0,
  );
}

export function countTotalFiles(plan: EditablePlan): number {
  return plan.folders.reduce((sum, folder) => sum + folder.files.length, 0);
}

export function countRenames(plan: EditablePlan): number {
  if (!plan.renameEnabled) return 0;
  return plan.folders.reduce(
    (sum, folder) =>
      sum +
      folder.files.filter(
        (f) => f.included && (f.editedName || f.suggestedName),
      ).length,
    0,
  );
}

export function hasAnyRenames(plan: EditablePlan): boolean {
  return plan.folders.some((folder) =>
    folder.files.some((f) => f.suggestedName),
  );
}

// Tree structure for nested folder display

export interface FolderTreeNode {
  name: string;
  path: string;
  children: FolderTreeNode[];
  folder: EditableFolder | null;
  fileCount: number;
  activeFileCount: number;
  renameCount: number;
}

export function buildFolderTree(plan: EditablePlan): FolderTreeNode[] {
  const nodeMap = new Map<string, FolderTreeNode>();

  const getOrCreate = (path: string): FolderTreeNode => {
    let node = nodeMap.get(path);
    if (!node) {
      const segments = path.split("/");
      node = {
        name: segments[segments.length - 1],
        path,
        children: [],
        folder: null,
        fileCount: 0,
        activeFileCount: 0,
        renameCount: 0,
      };
      nodeMap.set(path, node);
    }
    return node;
  };

  // Create nodes for each folder and all intermediate ancestors
  for (const folder of plan.folders) {
    const folderPath = folder.label.replace(/\/$/, "");
    const segments = folderPath.split("/");

    for (let i = 1; i <= segments.length; i++) {
      const partial = segments.slice(0, i).join("/");
      const node = getOrCreate(partial);

      // Link parent → child
      if (i > 1) {
        const parentPath = segments.slice(0, i - 1).join("/");
        const parent = getOrCreate(parentPath);
        if (!parent.children.some((c) => c.path === partial)) {
          parent.children.push(node);
        }
      }
    }

    // Attach the actual folder data to its leaf node
    const leafNode = nodeMap.get(folderPath)!;
    leafNode.folder = folder;
  }

  // Bottom-up aggregation
  const aggregate = (node: FolderTreeNode): void => {
    for (const child of node.children) {
      aggregate(child);
    }

    const childFileCount = node.children.reduce((s, c) => s + c.fileCount, 0);
    const childActiveCount = node.children.reduce((s, c) => s + c.activeFileCount, 0);
    const childRenameCount = node.children.reduce((s, c) => s + c.renameCount, 0);

    const ownFiles = node.folder?.files ?? [];
    const ownActive = ownFiles.filter((f) => f.included).length;
    const ownRenames = plan.renameEnabled
      ? ownFiles.filter((f) => f.included && (f.editedName || f.suggestedName)).length
      : 0;

    node.fileCount = childFileCount + ownFiles.length;
    node.activeFileCount = childActiveCount + ownActive;
    node.renameCount = childRenameCount + ownRenames;
  };

  // Collect root nodes (no parent) and sort
  const roots: FolderTreeNode[] = [];
  for (const node of nodeMap.values()) {
    const firstSlash = node.path.indexOf("/");
    if (firstSlash === -1) {
      roots.push(node);
    }
  }
  roots.sort((a, b) => a.name.localeCompare(b.name));

  // Sort children at every level, then aggregate
  const sortChildren = (node: FolderTreeNode): void => {
    node.children.sort((a, b) => a.name.localeCompare(b.name));
    for (const child of node.children) sortChildren(child);
  };
  for (const root of roots) {
    sortChildren(root);
    aggregate(root);
  }

  return roots;
}

// Serializer — reconstruct OrganizePlan shape for backend

export function toApprovedPlanJson(
  editPlan: EditablePlan,
  sourcePlan: OrganizePlan,
): string {
  const excludedPaths = new Set<string>();
  const nameEdits = new Map<string, string>();
  const folderEdits = new Map<string, string>();

  for (const folder of editPlan.folders) {
    if (folder.label !== folder.originalLabel + "/") {
      folderEdits.set(folder.originalLabel, folder.label);
    }
    for (const file of folder.files) {
      if (!file.included) excludedPaths.add(file.path);
      if (file.editedName) nameEdits.set(file.path, file.editedName);
    }
  }

  const applyFolderEdit = (folder: string): string => {
    for (const [original, edited] of folderEdits) {
      if (folder === original || folder === original + "/") {
        return edited.replace(/\/$/, "");
      }
    }
    return folder;
  };

  const filtered: OrganizePlan = {
    ...sourcePlan,
    categories: sourcePlan.categories
      .map((cat) => ({
        ...cat,
        folder: applyFolderEdit(cat.folder),
        files: cat.files.filter((f) => !excludedPaths.has(f)),
      }))
      .filter((cat) => cat.files.length > 0),
    placements: sourcePlan.placements
      ?.filter((p) => !excludedPaths.has(p.path))
      .map((placement) => {
        const result = { ...placement };
        result.folder = applyFolderEdit(result.folder);

        if (editPlan.renameEnabled) {
          const edited = nameEdits.get(placement.path);
          if (edited) {
            result.suggested_name = edited;
          }
        } else {
          delete result.suggested_name;
        }

        return result;
      }),
  };

  return JSON.stringify(filtered);
}
