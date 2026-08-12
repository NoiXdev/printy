import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { isPermissionGranted, sendNotification } from "@tauri-apps/plugin-notification";
import { api } from "./api";
import { onJobEvent, onQueueEvent } from "./events";
import { jobNotification, queueNotification, shouldNotify, type Toast } from "./notify";
import type { JobOutcome, NotificationMode, PrintJob } from "./types";

async function deliver(toast: Toast | null): Promise<void> {
  if (toast === null) return;
  // Only a check, never a prompt. Einstellungen asks for permission when the
  // user switches notifications on; asking here would interrupt the very print
  // this toast reports.
  if (!(await isPermissionGranted())) return;
  sendNotification({ title: toast.title, body: toast.body });
}

/**
 * The job event carries only the outcome, so the row it refers to is read back
 * from the ledger. A failure is looked up in the failed-only page; a success is
 * the newest `done` row. Taking whichever row happens to be newest would name
 * the wrong file as soon as another job was enqueued in between.
 */
async function jobFor(outcome: JobOutcome): Promise<PrintJob | null> {
  if (outcome === "Failed") {
    const rows = await api.listJobs(true, 1);
    return rows[0] ?? null;
  }
  const rows = await api.listJobs(false, 20);
  return rows.find((j) => j.state === "done") ?? null;
}

export function useJobNotifications(): void {
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });

  // The listeners are registered once; the mode is read through a ref so a
  // settings change takes effect without tearing down the subscription.
  const modeRef = useRef<NotificationMode>("all");
  modeRef.current = (settings.data?.notification_mode ?? "all") as NotificationMode;

  useEffect(() => {
    const jobSub = onJobEvent((e) => {
      if (!shouldNotify(modeRef.current, e.outcome)) return;
      void jobFor(e.outcome)
        .then((job) => deliver(jobNotification(e.outcome, job)))
        .catch(() => {
          // A toast is never worth breaking the app over.
        });
    });

    const queueSub = onQueueEvent((e) => {
      // A stalled printer is an error condition, so it passes the errors mode.
      if (modeRef.current === "off") return;
      void deliver(queueNotification(e)).catch(() => {});
    });

    return () => {
      void jobSub.then((un) => un());
      void queueSub.then((un) => un());
    };
  }, []);
}
