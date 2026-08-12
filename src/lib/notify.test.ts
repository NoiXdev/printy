import { describe, it, expect } from "vitest";
import { jobNotification, queueNotification, shouldNotify } from "./notify";
import type { PrintJob } from "./types";

function job(overrides: Partial<PrintJob> = {}): PrintJob {
  return {
    id: 1,
    folder_id: 1,
    file_path: "/Users/tim/Scans/rechnung.pdf",
    file_name: "rechnung.pdf",
    size_bytes: 10,
    mtime_ms: 1,
    sha256: "h",
    state: "done",
    attempts: 0,
    printer_name: "HP LaserJet",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    error_kind: null,
    error_message: null,
    next_attempt_at: null,
    enqueued_at: "2026-08-12 09:00:00",
    started_at: "2026-08-12 09:00:01",
    finished_at: "2026-08-12 09:00:05",
    // Added to PrintJob after this brief was written (per-folder fit-to-page
    // snapshot); included here only to satisfy the type.
    fit_to_page: 1,
    ...overrides,
  };
}

describe("shouldNotify", () => {
  it("says nothing at all when notifications are off", () => {
    expect(shouldNotify("off", "Printed")).toBe(false);
    expect(shouldNotify("off", "Failed")).toBe(false);
  });

  it("reports only failures in errors mode", () => {
    expect(shouldNotify("errors", "Printed")).toBe(false);
    expect(shouldNotify("errors", "Failed")).toBe(true);
  });

  it("reports prints and failures in all mode", () => {
    expect(shouldNotify("all", "Printed")).toBe(true);
    expect(shouldNotify("all", "Failed")).toBe(true);
  });

  it("never reports a retry — it is not an outcome the user must act on", () => {
    expect(shouldNotify("all", "Retried")).toBe(false);
    expect(shouldNotify("errors", "Retried")).toBe(false);
  });
});

describe("jobNotification", () => {
  it("names the file and the printer on success", () => {
    expect(jobNotification("Printed", job())).toEqual({
      title: "Gedruckt",
      body: "rechnung.pdf → HP LaserJet",
    });
  });

  it("names the file and the reason on failure", () => {
    expect(
      jobNotification("Failed", job({ state: "failed", error_message: "PDF nicht lesbar" })),
    ).toEqual({
      title: "Druck fehlgeschlagen",
      body: "rechnung.pdf: PDF nicht lesbar",
    });
  });

  it("falls back to a generic reason when the backend gave none", () => {
    expect(jobNotification("Failed", job({ state: "failed" }))).toEqual({
      title: "Druck fehlgeschlagen",
      body: "rechnung.pdf: Unbekannter Fehler",
    });
  });

  it("produces nothing for a retry or for a missing job", () => {
    expect(jobNotification("Retried", job())).toBeNull();
    expect(jobNotification("Printed", null)).toBeNull();
  });
});

describe("queueNotification", () => {
  it("announces a hold with its reason", () => {
    expect(queueNotification({ held: true, reason: "Drucker offline" })).toEqual({
      title: "Warteschlange angehalten",
      body: "Drucker offline. Printy versucht es jede Minute erneut.",
    });
  });

  it("says nothing when the queue resumes", () => {
    expect(queueNotification({ held: false })).toBeNull();
  });
});
