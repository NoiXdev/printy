import { useEffect, useState, type JSX } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { onFolderEvent, onJobEvent, onPausedEvent, onQueueEvent } from "../lib/events";
import { folderActivity } from "../lib/format";
import type { WatchFolder } from "../lib/types";
import FolderCard from "../components/FolderCard";
import "./screens.css";

const JOB_LIMIT = 300;

export default function Folders(): JSX.Element {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [queueHeld, setQueueHeld] = useState(false);
  const [holdReason, setHoldReason] = useState<string | null>(null);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [rescanMessage, setRescanMessage] = useState<string | null>(null);

  const folders = useQuery({ queryKey: ["folders"], queryFn: api.listFolders });
  const jobs = useQuery({
    queryKey: ["jobs", false, JOB_LIMIT],
    queryFn: () => api.listJobs(false, JOB_LIMIT),
  });
  const status = useQuery({ queryKey: ["status"], queryFn: api.getStatus });

  // The retry countdown has to tick without a backend push.
  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  // The database is the single source of truth: an event invalidates, it never
  // patches cached rows.
  useEffect(() => {
    const invalidate = (): void => {
      void qc.invalidateQueries({ queryKey: ["folders"] });
      void qc.invalidateQueries({ queryKey: ["jobs"] });
      void qc.invalidateQueries({ queryKey: ["status"] });
    };
    const unlisteners = [
      onJobEvent(invalidate),
      onFolderEvent(invalidate),
      onQueueEvent((e) => {
        setQueueHeld(e.held);
        setHoldReason(e.held ? (e.reason ?? "Drucker nicht erreichbar") : null);
        invalidate();
      }),
      // The user's pause toggle is its own event: it must refresh the
      // "Alles pausieren" state promptly without touching queueHeld/
      // holdReason, which only ever reflect the printer hold.
      onPausedEvent(invalidate),
    ];
    return () => {
      for (const p of unlisteners) void p.then((un) => un());
    };
  }, [qc]);

  const invalidateAll = (): void => {
    void qc.invalidateQueries({ queryKey: ["folders"] });
    void qc.invalidateQueries({ queryKey: ["jobs"] });
    void qc.invalidateQueries({ queryKey: ["status"] });
  };

  const setPaused = useMutation({
    mutationFn: (paused: boolean) => api.setGlobalPaused(paused),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["status"] }),
  });

  const toggleFolder = useMutation({
    mutationFn: (folder: WatchFolder) =>
      api.setFolderEnabled(folder.id, folder.enabled === 0),
    onSuccess: invalidateAll,
  });

  const scanNow = useMutation({
    mutationFn: (folder: WatchFolder) => api.scanNow(folder.id),
    onSuccess: invalidateAll,
  });

  const scanAllFolders = useMutation({
    mutationFn: () => api.scanAllFolders(),
    onSuccess: (result) => {
      invalidateAll();
      const files = result.enqueued === 1 ? "1 Datei" : `${result.enqueued} Dateien`;
      const scanned =
        result.folders_scanned === 1 ? "1 Ordner" : `${result.folders_scanned} Ordnern`;
      setRescanMessage(`${files} aus ${scanned} eingelesen.`);
    },
  });

  const userPaused = status.data?.user_paused ?? false;
  const busy = toggleFolder.isPending || scanNow.isPending;

  return (
    <section className="screen">
      <h1>Ordner</h1>

      <div className="aggregate">
        <span data-testid="aggregate">
          {status.data === undefined
            ? "Lade …"
            : `${status.data.active_folders} aktiv · ${status.data.printed_today} heute gedruckt`}
        </span>
        <button
          type="button"
          className="btn"
          disabled={scanAllFolders.isPending}
          onClick={() => {
            setRescanMessage(null);
            scanAllFolders.mutate();
          }}
        >
          {scanAllFolders.isPending ? "Ordner werden neu eingelesen …" : "Alle Ordner neu einlesen"}
        </button>
        <label className="switch-label" htmlFor="global-pause">
          <span className="switch">
            <input
              id="global-pause"
              type="checkbox"
              checked={userPaused}
              disabled={setPaused.isPending}
              onChange={(e) => setPaused.mutate(e.target.checked)}
            />
            <span className="switch-track" aria-hidden="true" />
          </span>
          <span>Alles pausieren</span>
        </label>
      </div>

      {rescanMessage && (
        <p className="helper" role="status" data-testid="rescan-message">
          {rescanMessage}
        </p>
      )}

      {queueHeld && (
        <p className="folder-error" role="status">
          Warteschlange angehalten: {holdReason}. Printy prüft den Drucker jede
          Minute erneut.
        </p>
      )}

      {folders.isLoading ? (
        <div className="loading">
          <span className="spinner" aria-hidden="true" />
          <span>Lade Ordner …</span>
        </div>
      ) : folders.data && folders.data.length > 0 ? (
        <ul className="folder-grid">
          {folders.data.map((f) => (
            <FolderCard
              key={f.id}
              folder={f}
              activity={folderActivity(jobs.data ?? [], f.id, nowMs)}
              queueHeld={queueHeld}
              nowMs={nowMs}
              busy={busy}
              onToggleEnabled={(folder) => toggleFolder.mutate(folder)}
              onScanNow={(folder) => scanNow.mutate(folder)}
              onEdit={(folder) => navigate(`/ordner/${folder.id}`)}
              onReveal={(folder) => void revealItemInDir(folder.path)}
            />
          ))}
        </ul>
      ) : (
        <div className="card empty">
          <p className="empty-title">Noch kein Ordner eingerichtet</p>
          <p className="empty-sub">
            Lege einen Ordner an, dann druckt Printy jede neue Datei darin
            automatisch.
          </p>
        </div>
      )}

      <button
        type="button"
        className="add-folder-tile"
        style={{ marginTop: "0.75rem" }}
        onClick={() => navigate("/ordner/neu")}
      >
        <span aria-hidden="true">+ </span>Ordner hinzufügen
      </button>
    </section>
  );
}
