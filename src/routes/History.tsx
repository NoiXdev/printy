import { useEffect, useState, type JSX } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";
import { onJobEvent } from "../lib/events";
import { formatDateTime, jobOutcomeLabel } from "../lib/format";
import type { JobState } from "../lib/types";
import "./screens.css";

const JOB_LIMIT = 300;

/** `reprint_job_cmd` enqueues a *new* job rather than reviving the old one, so
 * offering it on a job that has not finished yet would put the same file in the
 * queue twice. Only the two terminal states can be reprinted. */
function canReprint(state: JobState): boolean {
  return state === "done" || state === "failed";
}

export default function History(): JSX.Element {
  const qc = useQueryClient();
  const [onlyFailed, setOnlyFailed] = useState(false);

  // The filter is a query parameter rather than a predicate over a cached
  // array, so "Nur Fehler" stays correct past JOB_LIMIT: the backend picks the
  // 300 most recent *failures*, not the failures among the 300 most recent.
  const jobs = useQuery({
    queryKey: ["jobs", onlyFailed, JOB_LIMIT],
    queryFn: () => api.listJobs(onlyFailed, JOB_LIMIT),
    // Toggling the filter changes the query key, which would otherwise drop
    // back to the loading state and blank the list for a frame. Holding the
    // previous rows until the new ones arrive keeps the switch from flashing.
    placeholderData: (previous) => previous,
  });
  const folders = useQuery({ queryKey: ["folders"], queryFn: api.listFolders });

  useEffect(() => {
    const p = onJobEvent(() => {
      void qc.invalidateQueries({ queryKey: ["jobs"] });
    });
    return () => {
      void p.then((un) => un());
    };
  }, [qc]);

  const [reprintError, setReprintError] = useState<string | null>(null);
  const reprint = useMutation({
    mutationFn: (id: number) => api.reprintJob(id),
    onMutate: () => setReprintError(null),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["jobs"] });
      void qc.invalidateQueries({ queryKey: ["status"] });
    },
    // A reprint is refused when the file has been moved or deleted since.
    // That is the common case, not an edge one, and it needs saying.
    onError: (e) => setReprintError(String(e)),
  });

  // A job outlives the folder it came from, so the name cannot be assumed to
  // resolve -- history must stay readable after a folder is deleted.
  const folderName = (id: number): string =>
    folders.data?.find((f) => f.id === id)?.name ?? "Gelöschter Ordner";

  return (
    <section className="screen">
      <div className="screen-header">
        <h1>Verlauf</h1>
        <label className="switch-label" htmlFor="only-failed">
          <span className="switch">
            <input
              id="only-failed"
              type="checkbox"
              checked={onlyFailed}
              onChange={(e) => setOnlyFailed(e.target.checked)}
            />
            <span className="switch-track" aria-hidden="true" />
          </span>
          <span>Nur Fehler</span>
        </label>
      </div>

      <p className="helper" style={{ marginTop: 0 }}>
        „Gedruckt“ heißt: Der Auftrag wurde an den Drucker übergeben. Ob wirklich Papier
        herauskam, kann Printy nicht sehen.
      </p>

      {reprintError !== null && (
        <p className="folder-error" role="alert">
          {reprintError}
        </p>
      )}

      {jobs.isLoading ? (
        <div className="loading">
          <span className="spinner" aria-hidden="true" />
          <span>Lade Verlauf …</span>
        </div>
      ) : jobs.data && jobs.data.length > 0 ? (
        <ul className="history-list">
          {jobs.data.map((j) => (
            <li key={j.id} className="history-row" data-testid={`job-row-${j.id}`}>
              <div className="history-main">
                <span className="history-file">{j.file_name}</span>
                <span className="history-meta">
                  {folderName(j.folder_id)} · {j.printer_name}
                </span>
                {j.error_message !== null && (
                  <span className="folder-error">{j.error_message}</span>
                )}
              </div>
              <div className="history-right">
                <span className="history-time">
                  {formatDateTime(j.finished_at ?? j.enqueued_at)}
                </span>
                <span className={`badge ${j.state === "failed" ? "off" : "on"}`}>
                  {jobOutcomeLabel(j.state)}
                </span>
                {canReprint(j.state) && (
                  <button
                    type="button"
                    className="link-btn"
                    disabled={reprint.isPending}
                    onClick={() => reprint.mutate(j.id)}
                  >
                    Erneut drucken
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>
      ) : (
        <div className="card empty">
          <p className="empty-title">{onlyFailed ? "Keine Fehler" : "Noch nichts gedruckt"}</p>
          <p className="empty-sub">
            {onlyFailed
              ? "Bisher ist kein Auftrag fehlgeschlagen."
              : "Sobald eine Datei in einem überwachten Ordner landet, erscheint sie hier."}
          </p>
        </div>
      )}
    </section>
  );
}
