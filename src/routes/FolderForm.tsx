import { useState, type JSX } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { api } from "../lib/api";
import FolderDialog, { type FolderDialogResult } from "../components/FolderDialog";
import "./screens.css";

/**
 * Route for both "Ordner hinzufügen" and "Ordner bearbeiten". A missing `:id`
 * means create; a present one edits that row. All the data wiring that used
 * to live in Folders.tsx's dialog state moved here -- FolderDialog itself
 * stays a plain, prop-driven form so its existing tests keep working.
 */
export default function FolderForm(): JSX.Element {
  const { id } = useParams<{ id: string }>();
  const folderId = id !== undefined ? Number(id) : null;
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [capabilityPrinter, setCapabilityPrinter] = useState<string | null>(null);

  const folders = useQuery({ queryKey: ["folders"], queryFn: api.listFolders });
  const printers = useQuery({ queryKey: ["printers"], queryFn: api.listPrinters });
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const defaultPollIntervalSecs = Math.max(
    1,
    Number.parseInt(settings.data?.default_poll_interval_secs ?? "1", 10) || 1,
  );

  const folder =
    folderId !== null ? (folders.data?.find((f) => f.id === folderId) ?? null) : null;

  // The printer whose capabilities to probe: whatever the user just picked in
  // the dropdown, else the folder's own printer while editing, else the
  // system default -- the same fallback chain the old dialog state used.
  const activePrinter =
    capabilityPrinter ??
    folder?.printer_name ??
    printers.data?.find((p) => p.is_default)?.name ??
    printers.data?.[0]?.name ??
    null;

  const capabilities = useQuery({
    queryKey: ["capabilities", activePrinter],
    queryFn: () => api.printerCapabilities(activePrinter as string),
    enabled: activePrinter !== null,
  });

  const invalidateAll = (): void => {
    void qc.invalidateQueries({ queryKey: ["folders"] });
    void qc.invalidateQueries({ queryKey: ["jobs"] });
    void qc.invalidateQueries({ queryKey: ["status"] });
  };

  const save = useMutation({
    mutationFn: (result: FolderDialogResult) =>
      folderId === null
        ? api.createFolder(result.folder, result.printExisting)
        : api.updateFolder(folderId, result.folder),
    onSuccess: () => {
      invalidateAll();
      navigate("/");
    },
  });

  // While editing, the folder list hasn't arrived yet, so it is unknown
  // whether the id even exists. Either way, the printer list has to be in
  // hand before FolderDialog mounts: it computes its initial form state --
  // including the system-default preselection -- exactly once, on mount, so
  // mounting it against an empty list would freeze the printer field blank.
  if ((folderId !== null && folders.isLoading) || printers.isLoading) {
    return (
      <section className="screen">
        <div className="loading">
          <span className="spinner" aria-hidden="true" />
          <span>Lade …</span>
        </div>
      </section>
    );
  }

  // The folder list arrived and the id genuinely isn't in it -- deleted from
  // elsewhere, or a stale link. Nothing to edit, so send the user back rather
  // than rendering a form with no data behind it.
  if (folderId !== null && folders.data !== undefined && folder === null) {
    return (
      <section className="screen">
        <h1>Ordner nicht gefunden</h1>
        <p className="helper">
          Dieser Ordner wurde bereits gelöscht oder existiert nicht mehr.
        </p>
        <button type="button" className="btn" onClick={() => navigate("/")}>
          Zurück zur Ordnerliste
        </button>
      </section>
    );
  }

  return (
    <FolderDialog
      folder={folder}
      printers={printers.data ?? []}
      capabilities={capabilities.data ?? null}
      countExisting={(path, fileTypes) => api.countExistingFiles(path, fileTypes)}
      defaultPollIntervalSecs={defaultPollIntervalSecs}
      onPrinterChange={setCapabilityPrinter}
      onCancel={() => navigate("/")}
      onSubmit={(result) => save.mutate(result)}
      saving={save.isPending}
    />
  );
}
