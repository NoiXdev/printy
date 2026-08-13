import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom";
import { useState } from "react";
import FolderMenu, { type FolderMenuItem } from "./FolderMenu";

/** Mirrors how `FolderCard` actually uses `FolderMenu`: fully controlled. */
function Harness({ items }: { items: FolderMenuItem[] }) {
  const [open, setOpen] = useState(false);
  return (
    <FolderMenu label="Aktionen für Scanner" items={items} open={open} onOpenChange={setOpen} />
  );
}

function buildItems() {
  const handlers = {
    toggle: vi.fn(),
    scan: vi.fn(),
    edit: vi.fn(),
    reveal: vi.fn(),
    remove: vi.fn(),
  };
  const items: FolderMenuItem[] = [
    { key: "toggle", label: "Pausieren", onSelect: handlers.toggle, disabled: true },
    { key: "scan", label: "Jetzt scannen", onSelect: handlers.scan },
    { key: "edit", label: "Bearbeiten", onSelect: handlers.edit },
    { key: "reveal", label: "Im Explorer öffnen", onSelect: handlers.reveal },
    {
      key: "delete",
      label: "Löschen",
      onSelect: handlers.remove,
      destructive: true,
      separated: true,
    },
  ];
  return { items, handlers };
}

function openMenu(): void {
  fireEvent.click(screen.getByRole("button", { name: "Aktionen für Scanner" }));
}

describe("FolderMenu ARIA structure", () => {
  it("is closed by default and announces itself as a menu once opened", () => {
    const { items } = buildItems();
    render(<Harness items={items} />);
    const trigger = screen.getByRole("button", { name: "Aktionen für Scanner" });
    expect(trigger).toHaveAttribute("aria-haspopup", "menu");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    openMenu();

    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.getAllByRole("menuitem")).toHaveLength(5);
  });

  it("marks the delete item as destructive", () => {
    const { items } = buildItems();
    render(<Harness items={items} />);
    openMenu();
    expect(screen.getByRole("menuitem", { name: "Löschen" })).toHaveClass("destructive");
  });
});

describe("FolderMenu keyboard path", () => {
  it("opening focuses the first enabled item, skipping a disabled one", () => {
    const { items } = buildItems();
    render(<Harness items={items} />);
    openMenu();
    // "Pausieren" is disabled, so focus must skip straight to "Jetzt scannen".
    expect(screen.getByRole("menuitem", { name: "Jetzt scannen" })).toHaveFocus();
  });

  it("ArrowDown/ArrowUp move the roving focus, wrapping around and skipping disabled items", () => {
    const { items } = buildItems();
    render(<Harness items={items} />);
    openMenu();
    const menu = screen.getByRole("menu");

    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Bearbeiten" })).toHaveFocus();

    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Im Explorer öffnen" })).toHaveFocus();

    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Löschen" })).toHaveFocus();

    // Wraps back to the first enabled item, skipping disabled "Pausieren".
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Jetzt scannen" })).toHaveFocus();

    fireEvent.keyDown(menu, { key: "ArrowUp" });
    expect(screen.getByRole("menuitem", { name: "Löschen" })).toHaveFocus();
  });

  it("Enter activates the focused item, closes the menu and returns focus to the trigger", () => {
    const { items, handlers } = buildItems();
    render(<Harness items={items} />);
    const trigger = screen.getByRole("button", { name: "Aktionen für Scanner" });
    openMenu();

    fireEvent.keyDown(screen.getByRole("menu"), { key: "ArrowDown" }); // -> Bearbeiten
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Enter" });

    expect(handlers.edit).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("Space also activates the focused item", () => {
    const { items, handlers } = buildItems();
    render(<Harness items={items} />);
    openMenu();
    fireEvent.keyDown(screen.getByRole("menu"), { key: " " });
    expect(handlers.scan).toHaveBeenCalledTimes(1);
  });

  it("Escape closes the menu, returns focus to the trigger, and activates nothing", () => {
    const { items, handlers } = buildItems();
    render(<Harness items={items} />);
    const trigger = screen.getByRole("button", { name: "Aktionen für Scanner" });
    openMenu();

    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
    expect(Object.values(handlers).every((h) => h.mock.calls.length === 0)).toBe(true);
  });

  it("clicking a menu item activates it and closes the menu", () => {
    const { items, handlers } = buildItems();
    render(<Harness items={items} />);
    openMenu();
    fireEvent.click(screen.getByRole("menuitem", { name: "Im Explorer öffnen" }));
    expect(handlers.reveal).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("clicking a disabled item does nothing", () => {
    const { items, handlers } = buildItems();
    render(<Harness items={items} />);
    openMenu();
    fireEvent.click(screen.getByRole("menuitem", { name: "Pausieren" }));
    expect(handlers.toggle).not.toHaveBeenCalled();
    expect(screen.getByRole("menu")).toBeInTheDocument();
  });

  it("clicking outside the menu closes it", () => {
    const { items } = buildItems();
    render(
      <div>
        <Harness items={items} />
        <button type="button">Außerhalb</button>
      </div>,
    );
    openMenu();
    expect(screen.getByRole("menu")).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByRole("button", { name: "Außerhalb" }));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("clicking the trigger again toggles the menu closed", () => {
    const { items } = buildItems();
    render(<Harness items={items} />);
    const trigger = screen.getByRole("button", { name: "Aktionen für Scanner" });
    fireEvent.click(trigger);
    expect(screen.getByRole("menu")).toBeInTheDocument();
    fireEvent.click(trigger);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
