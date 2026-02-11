import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ContextMenu, type ContextMenuItem } from "./ContextMenu";

describe("ContextMenu", () => {
  const items: ContextMenuItem[] = [
    { label: "New Folder", action: vi.fn() },
    { label: "Rename", action: vi.fn() },
    { separator: true },
    { label: "Delete", action: vi.fn(), destructive: true },
  ];

  it("renders menu items", () => {
    render(<ContextMenu items={items} position={{ x: 100, y: 200 }} onClose={() => {}} />);
    expect(screen.getByText("New Folder")).toBeInTheDocument();
    expect(screen.getByText("Rename")).toBeInTheDocument();
    expect(screen.getByText("Delete")).toBeInTheDocument();
  });

  it("calls action and closes on click", () => {
    const onClose = vi.fn();
    render(<ContextMenu items={items} position={{ x: 100, y: 200 }} onClose={onClose} />);
    fireEvent.click(screen.getByText("New Folder"));
    expect(items[0].action).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("renders separator", () => {
    const { container } = render(
      <ContextMenu items={items} position={{ x: 100, y: 200 }} onClose={() => {}} />,
    );
    expect(container.querySelector("[data-separator]")).toBeInTheDocument();
  });

  it("styles destructive items differently", () => {
    render(<ContextMenu items={items} position={{ x: 100, y: 200 }} onClose={() => {}} />);
    const deleteButton = screen.getByText("Delete").closest("button")!;
    expect(deleteButton.className).toContain("text-red");
  });
});
