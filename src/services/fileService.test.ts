import { describe, it, expect, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { listDirectory } from "./fileService";
import type { FileEntry } from "../types/file";

const mockInvoke = vi.mocked(invoke);

describe("fileService", () => {
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

    expect(mockInvoke).toHaveBeenCalledWith("list_directory", { path: "/home/user" });
    expect(result).toEqual(mockEntries);
  });

  it("propagates errors from invoke", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("not a directory"));

    await expect(listDirectory("/invalid")).rejects.toThrow("not a directory");
  });
});
