import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SearchBar } from "./SearchBar";
import { useSearchStore } from "../../stores/searchStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../hooks/useSearch", () => ({
  useSearch: vi.fn(),
}));

const mockNavigateTo = vi.fn();
vi.mock("../../stores/fileStore", () => ({
  useFileStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ navigateTo: mockNavigateTo }),
}));

describe("SearchBar", () => {
  beforeEach(() => {
    useSearchStore.setState(useSearchStore.getInitialState());
    mockNavigateTo.mockClear();
  });

  it("renders nothing when closed", () => {
    const { container } = render(<SearchBar />);
    expect(container.querySelector("[data-testid='search-overlay']")).toBeNull();
  });

  it("renders overlay when open", () => {
    useSearchStore.getState().open();
    render(<SearchBar />);
    expect(screen.getByTestId("search-overlay")).toBeInTheDocument();
  });

  it("renders search input when open", () => {
    useSearchStore.getState().open();
    render(<SearchBar />);
    expect(screen.getByTestId("search-input")).toBeInTheDocument();
  });

  it("closes on Escape", () => {
    useSearchStore.getState().open();
    render(<SearchBar />);

    fireEvent.keyDown(screen.getByTestId("search-input"), { key: "Escape" });
    expect(useSearchStore.getState().isOpen).toBe(false);
  });

  it("closes on backdrop click", () => {
    useSearchStore.getState().open();
    render(<SearchBar />);

    fireEvent.click(screen.getByTestId("search-overlay"));
    expect(useSearchStore.getState().isOpen).toBe(false);
  });

  it("updates query on input change", () => {
    useSearchStore.getState().open();
    render(<SearchBar />);

    fireEvent.change(screen.getByTestId("search-input"), { target: { value: "hello" } });
    expect(useSearchStore.getState().query).toBe("hello");
  });

  it("displays results", () => {
    useSearchStore.setState({
      isOpen: true,
      query: "test",
      results: [
        {
          file_path: "/docs/readme.md",
          file_name: "readme.md",
          score: 0.9,
          match_source: "fts",
          snippet: null,
        },
        {
          file_path: "/src/app.ts",
          file_name: "app.ts",
          score: 0.7,
          match_source: "vec",
          snippet: null,
        },
      ],
    });
    render(<SearchBar />);

    expect(screen.getByText("readme.md")).toBeInTheDocument();
    expect(screen.getByText("app.ts")).toBeInTheDocument();
  });

  it("shows semantic badge for vec results", () => {
    useSearchStore.setState({
      isOpen: true,
      query: "test",
      results: [
        {
          file_path: "/src/app.ts",
          file_name: "app.ts",
          score: 0.7,
          match_source: "vec",
          snippet: null,
        },
      ],
    });
    render(<SearchBar />);

    expect(screen.getByText("semantic")).toBeInTheDocument();
  });

  it("does not show semantic badge for fts results", () => {
    useSearchStore.setState({
      isOpen: true,
      query: "test",
      results: [
        {
          file_path: "/docs/readme.md",
          file_name: "readme.md",
          score: 0.9,
          match_source: "fts",
          snippet: null,
        },
      ],
    });
    render(<SearchBar />);

    expect(screen.queryByText("semantic")).not.toBeInTheDocument();
  });

  it("navigates to parent dir on result click", () => {
    useSearchStore.setState({
      isOpen: true,
      query: "readme",
      results: [
        {
          file_path: "/docs/readme.md",
          file_name: "readme.md",
          score: 0.9,
          match_source: "fts",
          snippet: null,
        },
      ],
    });
    render(<SearchBar />);

    fireEvent.click(screen.getByText("readme.md"));
    expect(mockNavigateTo).toHaveBeenCalledWith("/docs");
    expect(useSearchStore.getState().isOpen).toBe(false);
  });

  it("ArrowDown moves selectedIndex forward", () => {
    useSearchStore.setState({
      isOpen: true,
      query: "test",
      results: [
        { file_path: "/a.txt", file_name: "a.txt", score: 1, match_source: "fts", snippet: null },
        { file_path: "/b.txt", file_name: "b.txt", score: 0.8, match_source: "fts", snippet: null },
      ],
    });
    render(<SearchBar />);

    fireEvent.keyDown(screen.getByTestId("search-input"), { key: "ArrowDown" });
    expect(useSearchStore.getState().selectedIndex).toBe(1);
  });

  it("ArrowUp moves selectedIndex backward", () => {
    useSearchStore.setState({
      isOpen: true,
      query: "test",
      selectedIndex: 1,
      results: [
        { file_path: "/a.txt", file_name: "a.txt", score: 1, match_source: "fts", snippet: null },
        { file_path: "/b.txt", file_name: "b.txt", score: 0.8, match_source: "fts", snippet: null },
      ],
    });
    render(<SearchBar />);

    fireEvent.keyDown(screen.getByTestId("search-input"), { key: "ArrowUp" });
    expect(useSearchStore.getState().selectedIndex).toBe(0);
  });

  it("ArrowDown clamps at last result", () => {
    useSearchStore.setState({
      isOpen: true,
      query: "test",
      selectedIndex: 0,
      results: [
        { file_path: "/a.txt", file_name: "a.txt", score: 1, match_source: "fts", snippet: null },
      ],
    });
    render(<SearchBar />);

    fireEvent.keyDown(screen.getByTestId("search-input"), { key: "ArrowDown" });
    expect(useSearchStore.getState().selectedIndex).toBe(0);
  });

  it("Enter selects highlighted result", () => {
    useSearchStore.setState({
      isOpen: true,
      query: "guide",
      selectedIndex: 0,
      results: [
        {
          file_path: "/docs/guide.md",
          file_name: "guide.md",
          score: 1,
          match_source: "hybrid",
          snippet: null,
        },
      ],
    });
    render(<SearchBar />);

    fireEvent.keyDown(screen.getByTestId("search-input"), { key: "Enter" });
    expect(mockNavigateTo).toHaveBeenCalledWith("/docs");
    expect(useSearchStore.getState().isOpen).toBe(false);
  });
});
