import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusBar } from "./StatusBar";

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

  it("displays current path when provided", () => {
    render(<StatusBar itemCount={5} currentPath="/Users/test" />);
    expect(screen.getByText("/Users/test")).toBeInTheDocument();
  });
});
