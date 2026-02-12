import { describe, it, expect, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { search } from "./searchService";
import type { SearchResult } from "../types/search";

const mockInvoke = vi.mocked(invoke);

describe("searchService", () => {
  it("calls invoke with correct command and args", async () => {
    const mockResults: SearchResult[] = [
      {
        file_path: "/docs/readme.md",
        file_name: "readme.md",
        score: 0.85,
        match_source: "fts",
        snippet: null,
      },
    ];
    mockInvoke.mockResolvedValueOnce(mockResults);

    const result = await search("readme", 10);

    expect(mockInvoke).toHaveBeenCalledWith("search", { query: "readme", limit: 10 });
    expect(result).toEqual(mockResults);
  });

  it("passes undefined limit when omitted", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    await search("test");

    expect(mockInvoke).toHaveBeenCalledWith("search", { query: "test", limit: undefined });
  });

  it("propagates errors from invoke", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("search failed"));

    await expect(search("bad")).rejects.toThrow("search failed");
  });
});
