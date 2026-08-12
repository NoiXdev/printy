import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom";
import SearchSelect from "./SearchSelect";

const opts = [
  { value: 1, label: "Lena" },
  { value: 2, label: "Max" },
  { value: 3, label: "Sophie" },
];

describe("SearchSelect", () => {
  it("shows placeholder when no value", () => {
    render(<SearchSelect value={null} onChange={() => {}} options={opts} placeholder="Wählen" />);
    expect(screen.getByText("Wählen")).toBeInTheDocument();
  });

  it("filters by query and selects via click", () => {
    const onChange = vi.fn();
    render(<SearchSelect value={null} onChange={onChange} options={opts} />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.change(screen.getByPlaceholderText("Suchen …"), { target: { value: "so" } });
    expect(screen.queryByText("Max")).not.toBeInTheDocument();
    fireEvent.mouseDown(screen.getByText("Sophie"));
    expect(onChange).toHaveBeenCalledWith(3);
  });

  it("selects active option with Enter", () => {
    const onChange = vi.fn();
    render(<SearchSelect value={null} onChange={onChange} options={opts} />);
    fireEvent.click(screen.getByRole("button"));
    const input = screen.getByPlaceholderText("Suchen …");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith(2);
  });

  it("closes the popup on Escape", () => {
    render(<SearchSelect value={null} onChange={() => {}} options={opts} />);
    fireEvent.click(screen.getByRole("button"));
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    fireEvent.keyDown(screen.getByPlaceholderText("Suchen …"), { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("is reachable and marked disabled to assistive technology, not removed", () => {
    render(
      <SearchSelect
        id="picker"
        value={2}
        onChange={() => {}}
        options={opts}
        ariaLabel="Empfänger"
        disabled
      />,
    );
    const control = screen.getByRole("button", { name: "Empfänger" });
    expect(control).toBeInTheDocument();
    expect(control).toBeDisabled();
  });

  it("cannot be opened or changed while disabled", () => {
    const onChange = vi.fn();
    render(
      <SearchSelect
        value={2}
        onChange={onChange}
        options={opts}
        ariaLabel="Empfänger"
        disabled
      />,
    );
    const control = screen.getByRole("button", { name: "Empfänger" });
    fireEvent.click(control);
    expect(control).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(onChange).not.toHaveBeenCalled();
  });
});
