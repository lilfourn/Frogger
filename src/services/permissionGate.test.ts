import { describe, it, expect, vi, beforeEach } from "vitest";

const mockCheckPermissionRequest = vi.fn();
const mockGetPermissionScopes = vi.fn();
const mockGetPermissionDefaults = vi.fn();
const mockResolvePermissionGrantTargets = vi.fn();
const mockNormalizePermissionScopes = vi.fn().mockResolvedValue({
  scanned: 0,
  normalized: 0,
  merged: 0,
  skipped: 0,
});
const mockUpsertPermissionScope = vi.fn();
const mockRequestPermissionPrompt = vi.fn();

vi.mock("./settingsService", () => ({
  checkPermissionRequest: (...args: unknown[]) => mockCheckPermissionRequest(...args),
  getPermissionScopes: () => mockGetPermissionScopes(),
  getPermissionDefaults: () => mockGetPermissionDefaults(),
  resolvePermissionGrantTargets: (...args: unknown[]) => mockResolvePermissionGrantTargets(...args),
  normalizePermissionScopes: () => mockNormalizePermissionScopes(),
  upsertPermissionScope: (...args: unknown[]) => mockUpsertPermissionScope(...args),
}));

vi.mock("../stores/permissionPromptStore", () => ({
  requestPermissionPrompt: (...args: unknown[]) => mockRequestPermissionPrompt(...args),
}));

import {
  confirmAllowOnceFallback,
  preflightPermission,
  retryPermissionAfterFailure,
} from "./permissionGate";

describe("permissionGate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetPermissionScopes.mockResolvedValue([]);
    mockGetPermissionDefaults.mockResolvedValue({
      content_scan_default: "ask",
      modification_default: "ask",
      ocr_default: "ask",
      indexing_default: "allow",
    });
    mockResolvePermissionGrantTargets.mockResolvedValue([]);
    mockNormalizePermissionScopes.mockResolvedValue({
      scanned: 0,
      normalized: 0,
      merged: 0,
      skipped: 0,
    });
  });

  it("returns false when preflight decision is allow", async () => {
    mockCheckPermissionRequest.mockResolvedValue({ decision: "allow", blocked: [] });

    const result = await preflightPermission("list_directory", ["/Users/test"], "Access directory");

    expect(result).toBe(false);
    expect(mockRequestPermissionPrompt).not.toHaveBeenCalled();
  });

  it("returns true when ask decision is allow once", async () => {
    mockCheckPermissionRequest.mockResolvedValue({
      decision: "ask",
      blocked: [
        {
          path: "/Users/test",
          capability: "content_scan",
          mode: "ask",
          scope_path: null,
        },
      ],
    });
    mockRequestPermissionPrompt.mockResolvedValue("allow_once");

    const result = await preflightPermission("list_directory", ["/Users/test"], "Access directory");

    expect(result).toBe(true);
    expect(mockUpsertPermissionScope).not.toHaveBeenCalled();
  });

  it("throws when user denies ask decision", async () => {
    mockCheckPermissionRequest.mockResolvedValue({
      decision: "ask",
      blocked: [
        {
          path: "/Users/test",
          capability: "content_scan",
          mode: "ask",
          scope_path: null,
        },
      ],
    });
    mockRequestPermissionPrompt.mockResolvedValue("deny");

    await expect(
      preflightPermission("list_directory", ["/Users/test"], "Access directory"),
    ).rejects.toThrow("Permission denied by user");
  });

  it("persists folder-level allow scopes when user selects always allow folder", async () => {
    const blocked = [
      {
        path: "/Users/test/docs/file.txt",
        capability: "content_scan",
        mode: "ask",
        scope_path: null,
      },
      {
        path: "/Users/test/docs/file.txt",
        capability: "modification",
        mode: "ask",
        scope_path: null,
      },
    ];

    mockCheckPermissionRequest.mockResolvedValue({ decision: "ask", blocked });
    mockResolvePermissionGrantTargets.mockResolvedValue([
      {
        path: "/Users/test/docs/file.txt",
        scope_path: null,
        folder_target: "/Users/test/docs",
        exact_target: "/Users/test/docs/file.txt",
        ambiguous: true,
      },
    ]);
    mockRequestPermissionPrompt.mockResolvedValue("always_allow_folder");

    const result = await preflightPermission(
      "move_files",
      ["/Users/test/docs/file.txt"],
      "Move files",
    );

    expect(result).toBe(true);
    expect(mockRequestPermissionPrompt).toHaveBeenCalledWith(
      expect.objectContaining({
        action: "move_files",
        promptKind: "initial",
        allowExactPath: true,
      }),
    );
    expect(mockUpsertPermissionScope).toHaveBeenCalledWith(
      "/Users/test/docs",
      "allow",
      "allow",
      "ask",
      "allow",
    );
    expect(mockNormalizePermissionScopes).toHaveBeenCalledTimes(1);
  });

  it("persists exact-path allow scopes when user selects always allow exact", async () => {
    mockCheckPermissionRequest.mockResolvedValue({
      decision: "ask",
      blocked: [
        {
          path: "/Users/test/docs/file.txt",
          capability: "content_scan",
          mode: "ask",
          scope_path: null,
        },
      ],
    });
    mockResolvePermissionGrantTargets.mockResolvedValue([
      {
        path: "/Users/test/docs/file.txt",
        scope_path: null,
        folder_target: "/Users/test/docs",
        exact_target: "/Users/test/docs/file.txt",
        ambiguous: true,
      },
    ]);
    mockRequestPermissionPrompt.mockResolvedValue("always_allow_exact");

    await preflightPermission(
      "read_file_text",
      ["/Users/test/docs/file.txt"],
      "Read file contents",
    );

    expect(mockUpsertPermissionScope).toHaveBeenCalledWith(
      "/Users/test/docs/file.txt",
      "allow",
      "ask",
      "ask",
      "allow",
    );
  });

  it("uses modal fallback for allow-once retry when post-failure check is ask", async () => {
    mockCheckPermissionRequest.mockResolvedValue({
      decision: "ask",
      blocked: [
        {
          path: "/Users/test",
          capability: "content_scan",
          mode: "ask",
          scope_path: null,
        },
      ],
    });
    mockRequestPermissionPrompt.mockResolvedValue("allow_once");

    await expect(
      retryPermissionAfterFailure("list_directory", ["/Users/test"], "Retry action"),
    ).resolves.toBe(true);
    expect(mockRequestPermissionPrompt).toHaveBeenCalledWith({
      title: "Retry action",
      action: "list_directory",
      promptKind: "retry",
      blocked: [
        {
          path: "/Users/test",
          capability: "content_scan",
          mode: "ask",
          scope_path: null,
        },
      ],
      allowAlways: false,
      allowExactPath: false,
    });
  });

  it("does not retry when post-failure check is not ask", async () => {
    mockCheckPermissionRequest.mockResolvedValue({ decision: "deny", blocked: [] });

    await expect(
      retryPermissionAfterFailure("list_directory", ["/System"], "Retry action"),
    ).resolves.toBe(false);
    expect(mockRequestPermissionPrompt).not.toHaveBeenCalled();
  });

  it("allows fallback prompt with explicit blocked list", async () => {
    mockRequestPermissionPrompt.mockResolvedValue("allow_once");

    await expect(
      confirmAllowOnceFallback("Retry action", "search", [
        {
          path: "/Users/test",
          capability: "content_scan",
          mode: "ask",
          scope_path: null,
        },
      ]),
    ).resolves.toBe(true);
  });
});
