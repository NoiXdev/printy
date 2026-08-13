import { describe, it, expect, vi } from "vitest";
import { closeExclusive, openExclusive } from "./menuCoordinator";

describe("menuCoordinator", () => {
  it("closes a previously opened menu when a different one opens", () => {
    const closeA = vi.fn();
    const closeB = vi.fn();

    openExclusive(closeA);
    expect(closeA).not.toHaveBeenCalled();

    openExclusive(closeB);
    expect(closeA).toHaveBeenCalledTimes(1);
    expect(closeB).not.toHaveBeenCalled();

    closeExclusive(closeB);
  });

  it("opening the same closer again does not call it", () => {
    const close = vi.fn();
    openExclusive(close);
    openExclusive(close);
    expect(close).not.toHaveBeenCalled();
    closeExclusive(close);
  });

  it("closeExclusive only clears the slot if it is the current owner", () => {
    const closeA = vi.fn();
    const closeB = vi.fn();

    openExclusive(closeA);
    openExclusive(closeB);
    // closeA is stale (already replaced) -- clearing it must not disturb B.
    closeExclusive(closeA);

    const closeC = vi.fn();
    openExclusive(closeC);
    // B is still the active owner, so opening C must have closed it.
    expect(closeB).toHaveBeenCalledTimes(1);
    closeExclusive(closeC);
  });
});
