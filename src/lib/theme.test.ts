import { describe, it, expect, beforeEach, vi } from "vitest";
import { applyTheme, getThemeChoice, resolveTheme } from "./theme";

describe("theme", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
  });

  it("defaults to system when nothing is stored", () => {
    expect(getThemeChoice()).toBe("system");
  });

  it("resolves an explicit choice without consulting the media query", () => {
    expect(resolveTheme("dark")).toBe("dark");
    expect(resolveTheme("light")).toBe("light");
  });

  it("resolves system from prefers-color-scheme", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({ matches: true, addEventListener: vi.fn() })),
    );
    expect(resolveTheme("system")).toBe("dark");
    vi.unstubAllGlobals();
  });

  it("applies the resolved theme to the root element and persists the choice", () => {
    applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(getThemeChoice()).toBe("dark");
  });

  it("rejects a corrupted stored value and falls back to system", () => {
    localStorage.setItem("printy-theme", "banana");
    expect(getThemeChoice()).toBe("system");
  });
});
