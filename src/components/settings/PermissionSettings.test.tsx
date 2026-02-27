import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { PermissionSettings } from "./PermissionSettings";

const mockGetPermissionScopes = vi.fn();
const mockGetPermissionDefaults = vi.fn();
const mockSetPermissionDefaults = vi.fn().mockResolvedValue(undefined);
const mockUpsertPermissionScope = vi.fn().mockResolvedValue(1);
const mockDeletePermissionScope = vi.fn().mockResolvedValue(1);

vi.mock("../../services/settingsService", () => ({
  getPermissionScopes: () => mockGetPermissionScopes(),
  getPermissionDefaults: () => mockGetPermissionDefaults(),
  setPermissionDefaults: (...args: unknown[]) => mockSetPermissionDefaults(...args),
  upsertPermissionScope: (...args: unknown[]) => mockUpsertPermissionScope(...args),
  deletePermissionScope: (...args: unknown[]) => mockDeletePermissionScope(...args),
}));

const ASK_DEFAULTS = {
  content_scan_default: "ask",
  modification_default: "ask",
  ocr_default: "ask",
  indexing_default: "allow",
};

const FULL_DEFAULTS = {
  content_scan_default: "allow",
  modification_default: "allow",
  ocr_default: "allow",
  indexing_default: "allow",
};

describe("PermissionSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetPermissionScopes.mockResolvedValue([]);
    mockGetPermissionDefaults.mockResolvedValue(ASK_DEFAULTS);
  });

  it("switches to full access and clears restrictive scopes", async () => {
    const restrictiveScope = {
      id: 1,
      directory_path: "/Users/test",
      content_scan_mode: "ask",
      modification_mode: "allow",
      ocr_mode: "allow",
      indexing_mode: "allow",
      created_at: "2026-01-01T00:00:00Z",
    };
    const fullyAllowedScope = {
      id: 2,
      directory_path: "/Users/test/Allowed",
      content_scan_mode: "allow",
      modification_mode: "allow",
      ocr_mode: "allow",
      indexing_mode: "allow",
      created_at: "2026-01-01T00:00:00Z",
    };

    mockGetPermissionScopes
      .mockResolvedValueOnce([restrictiveScope, fullyAllowedScope])
      .mockResolvedValueOnce([restrictiveScope, fullyAllowedScope])
      .mockResolvedValueOnce([fullyAllowedScope]);
    mockGetPermissionDefaults
      .mockResolvedValueOnce(ASK_DEFAULTS)
      .mockResolvedValueOnce(FULL_DEFAULTS);

    render(<PermissionSettings />);

    await waitFor(() => {
      expect(screen.getByTestId("permission-profile-full")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("permission-profile-full"));

    await waitFor(() => {
      expect(mockSetPermissionDefaults).toHaveBeenCalledWith(FULL_DEFAULTS);
      expect(mockDeletePermissionScope).toHaveBeenCalledWith(1);
    });

    expect(mockDeletePermissionScope).toHaveBeenCalledTimes(1);
  });

  it("switches to ask mode without deleting scopes", async () => {
    mockGetPermissionScopes.mockResolvedValue([]);
    mockGetPermissionDefaults
      .mockResolvedValueOnce(FULL_DEFAULTS)
      .mockResolvedValueOnce(ASK_DEFAULTS);

    render(<PermissionSettings />);

    await waitFor(() => {
      expect(screen.getByTestId("permission-profile-ask")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("permission-profile-ask"));

    await waitFor(() => {
      expect(mockSetPermissionDefaults).toHaveBeenCalledWith(ASK_DEFAULTS);
    });

    expect(mockDeletePermissionScope).not.toHaveBeenCalled();
  });

  it("shows an error message when profile update fails", async () => {
    mockSetPermissionDefaults.mockRejectedValueOnce(new Error("invoke failed"));

    render(<PermissionSettings />);

    await waitFor(() => {
      expect(screen.getByTestId("permission-profile-full")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("permission-profile-full"));

    await waitFor(() => {
      expect(screen.getByTestId("permission-profile-message")).toHaveTextContent(
        "Failed to update permission mode. Please try again.",
      );
    });
  });

  it("shows updated empty-state text", async () => {
    render(<PermissionSettings />);
    await waitFor(() => {
      expect(
        screen.getByText("No custom folder rules. Global mode controls access."),
      ).toBeInTheDocument();
    });
  });

  it("hides indexing controls from advanced rules", async () => {
    mockGetPermissionScopes.mockResolvedValue([
      {
        id: 1,
        directory_path: "/Users/test",
        content_scan_mode: "ask",
        modification_mode: "ask",
        ocr_mode: "ask",
        indexing_mode: "allow",
        created_at: "2026-01-01T00:00:00Z",
      },
    ]);

    render(<PermissionSettings />);

    await waitFor(() => {
      expect(screen.getByText("Content Scan")).toBeInTheDocument();
    });

    expect(screen.queryByText("Indexing")).not.toBeInTheDocument();
  });
});
