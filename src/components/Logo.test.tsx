import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import "@testing-library/jest-dom";
import Logo from "./Logo";

describe("Logo", () => {
  it("renders the mark at the requested size", () => {
    const { container } = render(<Logo size={48} />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg).toHaveAttribute("width", "48");
    expect(svg).toHaveAttribute("height", "48");
    expect(svg).toHaveAttribute("viewBox", "0 0 48 48");
  });

  it("hides the bare mark from assistive technology", () => {
    const { container } = render(<Logo />);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("renders the wordmark only when asked", () => {
    const { rerender } = render(<Logo />);
    expect(screen.queryByText("Printy")).not.toBeInTheDocument();
    rerender(<Logo wordmark />);
    expect(screen.getByText("Printy")).toBeInTheDocument();
  });

  it("colours the wordmark from the prop", () => {
    render(<Logo wordmark wordmarkColor="#f4f1ea" />);
    expect(screen.getByText("Printy")).toHaveStyle({ color: "#f4f1ea" });
  });
});
