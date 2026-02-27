import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsModal } from "./SettingsModal";

const mockSaveApiKey = vi.fn().mockResolvedValue(undefined);
const mockHasApiKey = vi.fn().mockResolvedValue(false);
const mockDeleteApiKey = vi.fn().mockResolvedValue(undefined);
const mockStartReembedIndexedFiles = vi.fn().mockResolvedValue({
  status: "done",
  processed: 10,
  total: 10,
  embedded: 9,
  skipped_missing: 1,
  failed: 0,
  message: "Rebuild complete: 9 embedded, 1 skipped missing, 0 failed.",
});
const mockGetReembedStatus = vi.fn().mockResolvedValue({
  status: "idle",
  processed: 0,
  total: 0,
  embedded: 0,
  skipped_missing: 0,
  failed: 0,
  message: "Idle",
});
const mockClearIndexedData = vi.fn().mockResolvedValue({
  files_removed: 42,
  ocr_removed: 5,
  fts_cleared: true,
  vec_removed: 30,
  vec_meta_removed: 30,
});
const mockStopIndexing = vi.fn().mockResolvedValue(undefined);
const mockStartIndexing = vi.fn().mockResolvedValue(undefined);
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
  getReembedStatus: () => mockGetReembedStatus(),
  startReembedIndexedFiles: () => mockStartReembedIndexedFiles(),
  clearIndexedData: () => mockClearIndexedData(),
  stopIndexing: () => mockStopIndexing(),
  startIndexing: (...args: unknown[]) => mockStartIndexing(...args),
  getPermissionScopes: () => mockGetPermissionScopes(),
  getPermissionDefaults: () => mockGetPermissionDefaults(),
  setPermissionDefaults: (...args: unknown[]) => mockSetPermissionDefaults(...args),
  upsertPermissionScope: (...args: unknown[]) => mockUpsertPermissionScope(...args),
  deletePermissionScope: (...args: unknown[]) => mockDeletePermissionScope(...args),
}));

const mockGetHomeDir = vi.fn().mockResolvedValue("/Users/testuser");

vi.mock("../../services/fileService", () => ({
  getHomeDir: () => mockGetHomeDir(),
}));

vi.mock("../../hooks/useTauriEvents", () => ({
  useTauriEvent: vi.fn(),
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
      expect(screen.getByTestId("key-status")).toHaveClass("bg-green-500");
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

  it("triggers re-embed and shows summary", async () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    await waitFor(() => {
      expect(screen.getByTestId("reembed-summary")).toHaveTextContent("Idle");
    });
    fireEvent.click(screen.getByTestId("reembed-now-btn"));
    await waitFor(() => {
      expect(mockStartReembedIndexedFiles).toHaveBeenCalled();
      expect(screen.getByTestId("reembed-summary")).toHaveTextContent(
        "Rebuild complete: 9 embedded, 1 skipped missing, 0 failed.",
      );
    });
  });

  it("clears indexed data and shows summary", async () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("clear-index-btn"));
    await waitFor(() => {
      expect(mockClearIndexedData).toHaveBeenCalled();
    });
    expect(screen.getByTestId("index-action-message")).toHaveTextContent(
      "Cleared 42 files, 30 vectors.",
    );
  });

  it("reindex stops then starts indexing", async () => {
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("reindex-btn"));
    await waitFor(() => {
      expect(mockStopIndexing).toHaveBeenCalled();
      expect(mockStartIndexing).toHaveBeenCalledWith("/Users/testuser");
    });
    expect(screen.getByTestId("index-action-message")).toHaveTextContent("Reindexing started.");
  });

  it("disables index buttons while clearing", async () => {
    let resolvePromise: () => void;
    mockClearIndexedData.mockReturnValue(
      new Promise<void>((resolve) => {
        resolvePromise = resolve;
      }),
    );
    render(<SettingsModal isOpen={true} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("clear-index-btn"));

    await waitFor(() => {
      expect(screen.getByTestId("clear-index-btn")).toBeDisabled();
      expect(screen.getByTestId("reindex-btn")).toBeDisabled();
    });

    resolvePromise!();
    await waitFor(() => {
      expect(screen.getByTestId("clear-index-btn")).not.toBeDisabled();
    });
  });
});
