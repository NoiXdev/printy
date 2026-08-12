import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import "@testing-library/jest-dom";
import type { ComponentProps } from "react";
import FolderCard from "./FolderCard";
import type { FolderActivity } from "../lib/format";
import type { WatchFolder } from "../lib/types";

const NOW = Date.parse("2026-08-12T12:00:00Z");

function folder(overrides: Partial<WatchFolder> = {}): WatchFolder {
  return {
    id: 1,
    name: "Scanner",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 5,
    file_types: '["pdf","png"]',
    printer_name: "Brother MFC",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    post_action: "move",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
    fit_to_page: 1,
    ...overrides,
  };
}

function activity(overrides: Partial<FolderActivity> = {}): FolderActivity {
  return {
    printedToday: 0,
    lastActivity: null,
    nextAttemptAt: null,
    failedCount: 0,
    ...overrides,
  };
}

function renderCard(props: Partial<ComponentProps<typeof FolderCard>> = {}) {
  const handlers = {
    onToggleEnabled: vi.fn(),
    onScanNow: vi.fn(),
    onEdit: vi.fn(),
    onReveal: vi.fn(),
  };
  render(
    <FolderCard
      folder={folder()}
      activity={activity()}
      queueHeld={false}
      nowMs={NOW}
      {...handlers}
      {...props}
    />,
  );
  return handlers;
}

describe("FolderCard status", () => {
  it("shows a mint running dot for an enabled healthy folder", () => {
    renderCard();
    expect(screen.getByTestId("status-dot")).toHaveClass("running");
    expect(screen.getByText("Aktiv")).toBeInTheDocument();
  });

  it("shows a grey paused dot for a disabled folder", () => {
    renderCard({ folder: folder({ enabled: 0 }) });
    expect(screen.getByTestId("status-dot")).toHaveClass("paused");
    expect(screen.getByText("Pausiert")).toBeInTheDocument();
  });

  it("shows a coral error dot and names the reason for a missing path", () => {
    renderCard({ folder: folder({ status: "path_missing" }) });
    expect(screen.getByTestId("status-dot")).toHaveClass("error");
    expect(screen.getByText("Ordner nicht gefunden")).toBeInTheDocument();
  });

  it("shows a coral error dot and names the reason for a missing printer", () => {
    renderCard({ folder: folder({ status: "printer_missing" }) });
    expect(screen.getByTestId("status-dot")).toHaveClass("error");
    expect(screen.getByText("Drucker nicht mehr installiert")).toBeInTheDocument();
  });

  it("distinguishes a printer hold from a user pause", () => {
    renderCard({ queueHeld: true });
    expect(screen.getByTestId("status-dot")).toHaveClass("waiting");
    expect(screen.getByText("Wartet auf Drucker")).toBeInTheDocument();
  });
});

describe("FolderCard content", () => {
  it("renders name, type badges, monospace path and settings summary", () => {
    renderCard();
    expect(screen.getByText("Scanner")).toBeInTheDocument();
    const badges = screen.getByTestId("type-badges");
    expect(within(badges).getByText("PDF")).toBeInTheDocument();
    expect(within(badges).getByText("PNG")).toBeInTheDocument();
    expect(screen.getByTestId("folder-path")).toHaveTextContent("/Users/tim/Scans");
    expect(screen.getByTestId("folder-path")).toHaveClass("mono");
    expect(
      screen.getByText("Brother MFC · 1 Kopie · Einseitig · Schwarz-weiß · verschieben"),
    ).toBeInTheDocument();
  });

  it("reports today's count and the last activity", () => {
    renderCard({
      activity: activity({ printedToday: 12, lastActivity: "2026-08-12 09:00:05" }),
    });
    expect(screen.getByText("12 heute gedruckt")).toBeInTheDocument();
    expect(screen.getByTestId("last-activity")).toHaveTextContent("12.08.2026");
  });

  it("counts down to the next retry when a job is held", () => {
    renderCard({ activity: activity({ nextAttemptAt: NOW + 30_000 }) });
    expect(screen.getByTestId("retry-countdown")).toHaveTextContent(
      "Nächster Versuch in 30 s",
    );
  });

  it("hides the countdown when nothing is pending", () => {
    renderCard();
    expect(screen.queryByTestId("retry-countdown")).not.toBeInTheDocument();
  });
});

describe("FolderCard actions", () => {
  it("offers pause, scan now, edit and reveal, and reports each one", () => {
    const h = renderCard();

    fireEvent.click(screen.getByRole("button", { name: "Pausieren" }));
    expect(h.onToggleEnabled).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Jetzt scannen" }));
    expect(h.onScanNow).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Bearbeiten" }));
    expect(h.onEdit).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Im Explorer öffnen" }));
    expect(h.onReveal).toHaveBeenCalledTimes(1);
  });

  it("offers resuming instead of pausing on a disabled folder", () => {
    renderCard({ folder: folder({ enabled: 0 }) });
    expect(screen.getByRole("button", { name: "Fortsetzen" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Pausieren" })).not.toBeInTheDocument();
  });

  it("locks every action while a mutation is in flight", () => {
    renderCard({ busy: true });
    expect(screen.getByRole("button", { name: "Pausieren" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Jetzt scannen" })).toBeDisabled();
  });
});
