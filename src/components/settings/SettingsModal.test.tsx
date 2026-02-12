import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsModal } from "./SettingsModal";

const mockSaveApiKey = vi.fn().mockResolvedValue(undefined);
const mockHasApiKey = vi.fn().mockResolvedValue(false);
const mockDeleteApiKey = vi.fn().mockResolvedValue(undefined);
const mockGetPermissionScopes = vi.fn().mockResolvedValue([]);
const mockGetPermissionDefaults = vi.fn().mockResolvedValue({
  content_scan_default: "ask",
  modification_default: "ask",
  ocr_default: "ask",
  indexing_default: "allow",
});
const mockSetPermissionDefaults = vi.fn().mockResolvedValue(undefined);
const mockUpsertPermissionScope = vi.fn().mockResolvedValue(1);
const mockDeletePermissionScope = vi.fn().mockResolvedValue(1);

vi.mock("../../services/settingsService", () => ({
  saveApiKey: (...args: unknown[]) => mockSaveApiKey(...args),
  hasApiKey: () => mockHasApiKey(),
  deleteApiKey: () => mockDeleteApiKey(),
  getPermissionScopes: () => mockGetPermissionScopes(),
  getPermissionDefaults: () => mockGetPermissionDefaults(),
  setPermissionDefaults: (...args: unknown[]) => mockSetPermissionDefaults(...args),
  upsertPermissionScope: (...args: unknown[]) => mockUpsertPermissionScope(...args),
  deletePermissionScope: (...args: unknown[]) => mockDeletePermissionScope(...args),
}));

describe("SettingsModal", () => {
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockHasApiKey.mockResolvedValue(false);
  });

  it("renders nothing when closed", () => {
    const { container } = render(<SettingsModal isOpen={false} onClose={onClose} />);
    expect(container.querySelector("[data-testid='settings-overlay']")).toBeNull();
  });

  it("renders overlay when open", () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    expect(screen.getByTestId("settings-overlay")).toBeInTheDocument();
  });

  it("closes on backdrop click", () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("settings-overlay"));
    expect(onClose).toHaveBeenCalled();
  });

  it("closes on Escape key", () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    fireEvent.keyDown(screen.getByTestId("settings-overlay"), { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("shows inactive status dot when no key", async () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    await waitFor(() => {
      expect(screen.getByTestId("key-status")).not.toHaveClass("bg-[var(--color-accent)]");
    });
  });

  it("shows active status dot when key exists", async () => {
    mockHasApiKey.mockResolvedValue(true);
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    await waitFor(() => {
      expect(screen.getByTestId("key-status")).toHaveClass("bg-[var(--color-accent)]");
    });
  });

  it("saves API key on button click", async () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    fireEvent.change(screen.getByTestId("api-key-input"), { target: { value: "sk-test-123" } });
    fireEvent.click(screen.getByTestId("save-key-btn"));
    await waitFor(() => {
      expect(mockSaveApiKey).toHaveBeenCalledWith("sk-test-123");
    });
  });

  it("saves API key on Enter", async () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    const input = screen.getByTestId("api-key-input");
    fireEvent.change(input, { target: { value: "sk-test-456" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(mockSaveApiKey).toHaveBeenCalledWith("sk-test-456");
    });
  });

  it("shows remove button when key exists", async () => {
    mockHasApiKey.mockResolvedValue(true);
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    await waitFor(() => {
      expect(screen.getByTestId("delete-key-btn")).toBeInTheDocument();
    });
  });

  it("deletes API key on remove click", async () => {
    mockHasApiKey.mockResolvedValue(true);
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    await waitFor(() => {
      expect(screen.getByTestId("delete-key-btn")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId("delete-key-btn"));
    await waitFor(() => {
      expect(mockDeleteApiKey).toHaveBeenCalled();
    });
  });

  it("disables save when input is empty", () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    expect(screen.getByTestId("save-key-btn")).toBeDisabled();
  });
});
