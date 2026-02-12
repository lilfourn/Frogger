import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("./permissionGate", () => ({
  preflightPermission: vi.fn().mockResolvedValue(false),
  retryPermissionAfterFailure: vi.fn().mockResolvedValue(false),
}));

import { invoke } from "@tauri-apps/api/core";
import { listDirectory } from "./fileService";
import type { FileEntry } from "../types/file";
import { preflightPermission, retryPermissionAfterFailure } from "./permissionGate";

const mockInvoke = vi.mocked(invoke);
const mockPreflightPermission = vi.mocked(preflightPermission);
const mockRetryPermissionAfterFailure = vi.mocked(retryPermissionAfterFailure);

describe("fileService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPreflightPermission.mockResolvedValue(false);
    mockRetryPermissionAfterFailure.mockResolvedValue(false);
  });

  it("calls invoke with correct command and args", async () => {
    const mockEntries: FileEntry[] = [
      {
        path: "/home/user/docs",
        name: "docs",
        extension: null,
        mime_type: null,
        size_bytes: null,
        created_at: null,
        modified_at: null,
        is_directory: true,
        parent_path: "/home/user",
      },
    ];
    mockInvoke.mockResolvedValueOnce(mockEntries);

    const result = await listDirectory("/home/user");

    expect(mockInvoke).toHaveBeenCalledWith("list_directory", {
      path: "/home/user",
      allowOnce: false,
    });
    expect(result).toEqual(mockEntries);
  });

  it("propagates errors from invoke", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("not a directory"));

    await expect(listDirectory("/invalid")).rejects.toThrow("not a directory");
  });

  it("retries with allowOnce when fallback is approved", async () => {
    mockPreflightPermission.mockResolvedValueOnce(false);
    mockInvoke.mockRejectedValueOnce(new Error("permission failed")).mockResolvedValueOnce([]);
    mockRetryPermissionAfterFailure.mockResolvedValueOnce(true);

    await expect(listDirectory("/home/user")).resolves.toEqual([]);
    expect(mockInvoke).toHaveBeenNthCalledWith(1, "list_directory", {
      path: "/home/user",
      allowOnce: false,
    });
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "list_directory", {
      path: "/home/user",
      allowOnce: true,
    });
    expect(mockRetryPermissionAfterFailure).toHaveBeenCalledWith(
      "list_directory",
      ["/home/user"],
      "Access directory contents",
    );
  });
});
