import type { JobOutcome, NotificationMode, PrintJob, QueueEvent } from "./types";

export interface Toast {
  title: string;
  body: string;
}

/**
 * A retry is never announced. It is a transient step the queue takes on its own,
 * and three toasts on the way to one failure would train the user to ignore them.
 */
export function shouldNotify(mode: NotificationMode, outcome: JobOutcome): boolean {
  if (outcome === "Retried") return false;
  switch (mode) {
    case "off":
      return false;
    case "errors":
      return outcome === "Failed";
    case "all":
      return true;
  }
}

export function jobNotification(outcome: JobOutcome, job: PrintJob | null): Toast | null {
  if (job === null) return null;
  if (outcome === "Printed") {
    return { title: "Gedruckt", body: `${job.file_name} → ${job.printer_name}` };
  }
  if (outcome === "Failed") {
    return {
      title: "Druck fehlgeschlagen",
      body: `${job.file_name}: ${job.error_message ?? "Unbekannter Fehler"}`,
    };
  }
  return null;
}

export function queueNotification(event: QueueEvent): Toast | null {
  if (!event.held) return null;
  return {
    title: "Warteschlange angehalten",
    body: `${event.reason ?? "Drucker nicht erreichbar"}. Printy versucht es jede Minute erneut.`,
  };
}
