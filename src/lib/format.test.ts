import { describe, it, expect } from "vitest";
import {
  chipIdsFromFileTypes,
  countdownSeconds,
  fileTypeLabels,
  fileTypesFromChipIds,
  folderActivity,
  folderDeleteWarning,
  folderStatusKind,
  folderStatusLabel,
  formatDateTime,
  jobOutcomeLabel,
  parseDbTimestamp,
  parseFileTypes,
  settingsSummary,
  stabilityHint,
} from "./format";
import type { PrintJob, WatchFolder } from "./types";

function folder(overrides: Partial<WatchFolder> = {}): WatchFolder {
  return {
    id: 1,
    name: "Scans",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 5,
    file_types: '["pdf","png"]',
    printer_name: "Brother MFC",
    copies: 2,
    duplex: "long_edge",
    color_mode: "mono",
    post_action: "move",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
    fit_to_page: 1,
    ...overrides,
  };
}

function job(overrides: Partial<PrintJob> = {}): PrintJob {
  return {
    id: 1,
    folder_id: 1,
    file_path: "/Users/tim/Scans/a.pdf",
    file_name: "a.pdf",
    size_bytes: 1024,
    mtime_ms: 1_700_000_000_000,
    sha256: "h",
    state: "done",
    attempts: 0,
    printer_name: "Brother MFC",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    error_kind: null,
    error_message: null,
    next_attempt_at: null,
    enqueued_at: "2026-08-12 09:00:00",
    started_at: "2026-08-12 09:00:01",
    finished_at: "2026-08-12 09:00:05",
    fit_to_page: 1,
    ...overrides,
  };
}

describe("parseDbTimestamp", () => {
  it("reads sqlite datetime('now') as UTC, not as local time", () => {
    const d = parseDbTimestamp("2026-08-12 09:00:05");
    expect(d?.toISOString()).toBe("2026-08-12T09:00:05.000Z");
  });

  it("returns null for null and for junk", () => {
    expect(parseDbTimestamp(null)).toBeNull();
    expect(parseDbTimestamp("nicht ein datum")).toBeNull();
  });
});

describe("formatDateTime", () => {
  it("renders an em dash when there is no timestamp", () => {
    expect(formatDateTime(null)).toBe("—");
  });

  it("renders a German date and time otherwise", () => {
    expect(formatDateTime("2026-08-12 09:00:05")).toMatch(/12\.08\.2026/);
  });
});

describe("file types", () => {
  it("parses the JSON column and survives corruption", () => {
    expect(parseFileTypes('["pdf","png"]')).toEqual(["pdf", "png"]);
    expect(parseFileTypes("nope")).toEqual([]);
  });

  it("maps extensions onto chips and back", () => {
    expect(chipIdsFromFileTypes(["pdf", "jpg", "jpeg"])).toEqual(["pdf", "jpg"]);
    expect(fileTypesFromChipIds(["jpg", "tiff"])).toEqual(["jpg", "jpeg", "tif", "tiff"]);
  });

  it("labels the chips for display", () => {
    expect(fileTypeLabels(["pdf", "tif", "tiff"])).toEqual(["PDF", "TIFF"]);
  });
});

describe("folderStatusKind", () => {
  it("reports running for an enabled healthy folder", () => {
    expect(folderStatusKind(folder(), false)).toBe("running");
  });

  it("reports paused for a disabled folder even when the queue is held", () => {
    expect(folderStatusKind(folder({ enabled: 0 }), true)).toBe("paused");
  });

  it("reports error for a folder whose path or printer is gone", () => {
    expect(folderStatusKind(folder({ status: "path_missing" }), false)).toBe("error");
    expect(folderStatusKind(folder({ status: "printer_missing" }), false)).toBe("error");
  });

  it("reports waiting when the queue is held on a healthy enabled folder", () => {
    expect(folderStatusKind(folder(), true)).toBe("waiting");
  });

  it("labels every kind in German", () => {
    expect(folderStatusLabel("running", folder())).toBe("Aktiv");
    expect(folderStatusLabel("paused", folder({ enabled: 0 }))).toBe("Pausiert");
    expect(folderStatusLabel("waiting", folder())).toBe("Wartet auf Drucker");
    expect(folderStatusLabel("error", folder({ status: "path_missing" }))).toBe(
      "Ordner nicht gefunden",
    );
    expect(folderStatusLabel("error", folder({ status: "printer_missing" }))).toBe(
      "Drucker nicht mehr installiert",
    );
  });
});

describe("settingsSummary", () => {
  it("summarises printer and print options in German", () => {
    expect(settingsSummary(folder())).toBe(
      "Brother MFC · 2 Kopien · Duplex (lange Kante) · Schwarz-weiß · verschieben",
    );
  });

  it("uses the singular for a single copy and names simplex plainly", () => {
    expect(settingsSummary(folder({ copies: 1, duplex: "simplex", color_mode: "color" }))).toBe(
      "Brother MFC · 1 Kopie · Einseitig · Farbe · verschieben",
    );
  });
});

describe("folderActivity", () => {
  const now = Date.parse("2026-08-12T12:00:00Z");

  it("counts today's completed jobs for this folder only", () => {
    const jobs = [
      job({ id: 1, folder_id: 1, finished_at: "2026-08-12 09:00:05" }),
      job({ id: 2, folder_id: 1, finished_at: "2026-08-12 10:00:05" }),
      job({ id: 3, folder_id: 2, finished_at: "2026-08-12 10:30:05" }),
      job({ id: 4, folder_id: 1, finished_at: "2026-08-11 10:00:05" }),
    ];
    expect(folderActivity(jobs, 1, now).printedToday).toBe(2);
  });

  it("reports the most recent finish as the last activity", () => {
    const jobs = [
      job({ id: 1, folder_id: 1, finished_at: "2026-08-12 09:00:05" }),
      job({ id: 2, folder_id: 1, finished_at: "2026-08-12 10:00:05" }),
    ];
    expect(folderActivity(jobs, 1, now).lastActivity).toBe("2026-08-12 10:00:05");
  });

  it("surfaces the earliest pending retry so the card can count down", () => {
    const jobs = [
      job({ id: 1, folder_id: 1, state: "retrying", next_attempt_at: now + 30_000 }),
      job({ id: 2, folder_id: 1, state: "retrying", next_attempt_at: now + 10_000 }),
    ];
    expect(folderActivity(jobs, 1, now).nextAttemptAt).toBe(now + 10_000);
  });

  it("is empty when the folder has no jobs at all", () => {
    expect(folderActivity([], 1, now)).toEqual({
      printedToday: 0,
      lastActivity: null,
      nextAttemptAt: null,
      failedCount: 0,
    });
  });
});

describe("countdownSeconds", () => {
  it("rounds up to whole seconds and never goes negative", () => {
    expect(countdownSeconds(1000, 0)).toBe(1);
    expect(countdownSeconds(1500, 0)).toBe(2);
    expect(countdownSeconds(500, 1000)).toBe(0);
    expect(countdownSeconds(null, 0)).toBeNull();
  });
});

describe("stabilityHint", () => {
  it("names the two-tick cost in seconds", () => {
    expect(stabilityHint(5)).toContain("10 Sekunden");
  });

  it("switches to minutes once the delay passes a minute", () => {
    expect(stabilityHint(60)).toContain("2 Minuten");
  });
});

describe("jobOutcomeLabel", () => {
  it("labels every job state in German", () => {
    expect(jobOutcomeLabel("done")).toBe("Gedruckt");
    expect(jobOutcomeLabel("failed")).toBe("Fehlgeschlagen");
    expect(jobOutcomeLabel("printing")).toBe("Druckt");
    expect(jobOutcomeLabel("queued")).toBe("Wartet");
    expect(jobOutcomeLabel("retrying")).toBe("Wiederholt");
  });
});

describe("folderDeleteWarning", () => {
  it("names both counts concretely, matching the spec's own example", () => {
    expect(folderDeleteWarning({ waiting: 3, history: 47 })).toBe(
      "3 wartende Aufträge und 47 Einträge im Verlauf werden mitgelöscht. " +
        "Die Dateien im überwachten Ordner selbst werden dabei nicht angetastet.",
    );
  });

  it("uses the singular form for exactly one of each", () => {
    const text = folderDeleteWarning({ waiting: 1, history: 1 });
    expect(text).toContain("1 wartender Auftrag und 1 Eintrag im Verlauf werden mitgelöscht.");
  });

  it("says so plainly when both counts are zero, instead of printing zeros", () => {
    const text = folderDeleteWarning({ waiting: 0, history: 0 });
    expect(text).not.toContain("0 ");
    expect(text).toContain("weder wartende Aufträge noch Einträge im Verlauf");
  });

  it("still names the non-zero side when only waiting jobs are zero", () => {
    const text = folderDeleteWarning({ waiting: 0, history: 47 });
    expect(text).not.toContain("0 wartende");
    expect(text).toContain("Keine wartenden Aufträge, aber 47 Einträge im Verlauf werden mitgelöscht.");
  });

  it("still names the non-zero side when only history is zero", () => {
    const text = folderDeleteWarning({ waiting: 3, history: 0 });
    expect(text).not.toContain("0 Einträge");
    expect(text).toContain("3 wartende Aufträge werden mitgelöscht, aber kein Eintrag im Verlauf.");
  });

  it("always states that the watched folder's own files are untouched", () => {
    for (const impact of [
      { waiting: 0, history: 0 },
      { waiting: 3, history: 47 },
      { waiting: 0, history: 5 },
      { waiting: 5, history: 0 },
    ]) {
      expect(folderDeleteWarning(impact)).toContain(
        "Die Dateien im überwachten Ordner selbst werden dabei nicht angetastet.",
      );
    }
  });
});
