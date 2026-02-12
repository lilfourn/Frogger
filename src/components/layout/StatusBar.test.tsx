import { describe, it, expect, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { listen } from "@tauri-apps/api/event";
import { StatusBar } from "./StatusBar";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

describe("StatusBar", () => {
  it("renders status bar", () => {
    render(<StatusBar itemCount={0} />);
    expect(screen.getByTestId("status-bar")).toBeInTheDocument();
  });

  it("displays item count", () => {
    render(<StatusBar itemCount={42} />);
    expect(screen.getByText("42 items")).toBeInTheDocument();
  });

  it("displays singular item for count of 1", () => {
    render(<StatusBar itemCount={1} />);
    expect(screen.getByText("1 item")).toBeInTheDocument();
  });

  it("displays zero items", () => {
    render(<StatusBar itemCount={0} />);
    expect(screen.getByText("0 items")).toBeInTheDocument();
  });

  it("displays branding", () => {
    render(<StatusBar itemCount={5} />);
    expect(screen.getByText("Frogger")).toBeInTheDocument();
  });

  it("shows indexing progress with spinner", async () => {
    let eventCallback: (event: { payload: unknown }) => void = () => {};
    vi.mocked(listen).mockImplementation((_event, handler) => {
      eventCallback = handler as typeof eventCallback;
      return Promise.resolve(vi.fn());
    });

    render(<StatusBar itemCount={3} />);
    await act(async () => {});

    expect(screen.queryByTestId("indexing-indicator")).not.toBeInTheDocument();

    act(() => {
      eventCallback({ payload: { processed: 50, total: 200, status: "active" } });
    });

    expect(screen.getByTestId("indexing-indicator")).toBeInTheDocument();
    expect(screen.getByText("50/200 files indexed")).toBeInTheDocument();
  });

  it("hides indexing indicator when done", async () => {
    let eventCallback: (event: { payload: unknown }) => void = () => {};
    vi.mocked(listen).mockImplementation((_event, handler) => {
      eventCallback = handler as typeof eventCallback;
      return Promise.resolve(vi.fn());
    });

    render(<StatusBar itemCount={3} />);
    await act(async () => {});

    act(() => {
      eventCallback({ payload: { processed: 10, total: 100, status: "active" } });
    });
    expect(screen.getByTestId("indexing-indicator")).toBeInTheDocument();

    act(() => {
      eventCallback({ payload: { processed: 100, total: 100, status: "done" } });
    });
    expect(screen.queryByTestId("indexing-indicator")).not.toBeInTheDocument();
  });
});
