import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("./permissionGate", () => ({
  preflightPermission: vi.fn().mockResolvedValue(false),
  retryPermissionAfterFailure: vi.fn().mockResolvedValue(false),
}));

import { invoke } from "@tauri-apps/api/core";
import { search } from "./searchService";
import type { SearchResult } from "../types/search";
import { preflightPermission, retryPermissionAfterFailure } from "./permissionGate";

const mockInvoke = vi.mocked(invoke);
const mockPreflightPermission = vi.mocked(preflightPermission);
const mockRetryPermissionAfterFailure = vi.mocked(retryPermissionAfterFailure);

describe("searchService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPreflightPermission.mockResolvedValue(false);
    mockRetryPermissionAfterFailure.mockResolvedValue(false);
  });

  it("calls invoke with correct command and args", async () => {
    const mockResults: SearchResult[] = [
      {
        file_path: "/docs/readme.md",
        file_name: "readme.md",
        is_directory: false,
        score: 0.85,
        match_source: "fts",
        snippet: null,
      },
    ];
    mockInvoke.mockResolvedValueOnce(mockResults);

    const result = await search("readme", 10);

    expect(mockInvoke).toHaveBeenCalledWith("search", {
      query: "readme",
      limit: 10,
      allowOnce: false,
    });
    expect(result).toEqual(mockResults);
  });

  it("passes undefined limit when omitted", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    await search("test");

    expect(mockInvoke).toHaveBeenCalledWith("search", {
      query: "test",
      limit: undefined,
      allowOnce: false,
    });
  });

  it("propagates errors from invoke", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("search failed"));

    await expect(search("bad")).rejects.toThrow("search failed");
  });

  it("retries with allowOnce when fallback is approved", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("permission failed")).mockResolvedValueOnce([]);
    mockRetryPermissionAfterFailure.mockResolvedValueOnce(true);

    await expect(search("frogger", 5, "/Users/test")).resolves.toEqual([]);
    expect(mockInvoke).toHaveBeenNthCalledWith(1, "search", {
      query: "frogger",
      limit: 5,
      allowOnce: false,
    });
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "search", {
      query: "frogger",
      limit: 5,
      allowOnce: true,
    });
    expect(mockRetryPermissionAfterFailure).toHaveBeenCalledWith(
      "search",
      ["/Users/test"],
      "Search indexed files",
    );
  });
});
