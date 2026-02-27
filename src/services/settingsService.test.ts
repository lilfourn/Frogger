import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { setPermissionDefaults } from "./settingsService";

const mockInvoke = vi.mocked(invoke);

describe("settingsService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("sends camelCase args when setting permission defaults", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await setPermissionDefaults({
      content_scan_default: "allow",
      modification_default: "allow",
      ocr_default: "allow",
      indexing_default: "allow",
    });

    expect(mockInvoke).toHaveBeenCalledWith("set_permission_defaults", {
      contentScanDefault: "allow",
      modificationDefault: "allow",
      ocrDefault: "allow",
      indexingDefault: "allow",
    });
  });
});
