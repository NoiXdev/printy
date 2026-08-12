import { useEffect, useRef, useState, type JSX } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { appDataDir, join } from "@tauri-apps/api/path";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { applyTheme, getThemeChoice, type ThemeChoice } from "../lib/theme";
import type { NotificationMode, SettingKey } from "../lib/types";
import "./screens.css";

const NOTIFICATION_OPTIONS: ReadonlyArray<{ value: NotificationMode; label: string }> = [
  { value: "all", label: "Alle" },
  { value: "errors", label: "Nur Fehler" },
  { value: "off", label: "Aus" },
];

const THEME_OPTIONS: ReadonlyArray<{ value: ThemeChoice; label: string }> = [
  { value: "light", label: "Hell" },
  { value: "dark", label: "Dunkel" },
  { value: "system", label: "System" },
];

export default function Settings(): JSX.Element {
  const qc = useQueryClient();
  const [theme, setTheme] = useState<ThemeChoice>(() => getThemeChoice());
  const [sumatra, setSumatra] = useState("");
  const [dbPath, setDbPath] = useState<string | null>(null);
  const [permissionDenied, setPermissionDenied] = useState(false);

  const settings = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const autostart = useQuery({ queryKey: ["autostart"], queryFn: api.getAutostart });

  // Guards against the settings fetch resolving after the user has already
  // started typing: once the field is touched, a late arrival (or a refetch
  // triggered by the mutation's own success handler) must never clobber it.
  const sumatraTouched = useRef(false);
  useEffect(() => {
    if (settings.data !== undefined && !sumatraTouched.current) {
      setSumatra(settings.data.sumatra_path);
    }
  }, [settings.data]);

  useEffect(() => {
    void appDataDir()
      .then((dir) => join(dir, "printy.sqlite"))
      .then(setDbPath)
      .catch(() => setDbPath(null));
  }, []);

  const updateSetting = useMutation({
    mutationFn: (vars: { key: SettingKey; value: string }) =>
      api.updateSetting(vars.key, vars.value),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["settings"] }),
  });

  const setAutostart = useMutation({
    mutationFn: (enabled: boolean) => api.setAutostart(enabled),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["autostart"] }),
  });

  const notificationMode = (settings.data?.notification_mode ?? "all") as NotificationMode;
  const startMinimized = settings.data?.start_minimized === "1";

  function handleTheme(choice: ThemeChoice): void {
    // localStorage only. The theme has to be applied before the first paint,
    // which an asynchronous settings read cannot do, so a second copy in the
    // database would only be a second thing to disagree with.
    applyTheme(choice);
    setTheme(choice);
  }

  /**
   * Turning notifications on is the moment to ask for permission. Prompting on
   * the first toast would interrupt the very print it reports, and a mode that
   * reads "Alle" while the OS drops every toast would be a lie — so a refusal
   * snaps the setting back to "Aus".
   */
  async function handleNotificationMode(mode: NotificationMode): Promise<void> {
    setPermissionDenied(false);
    if (mode !== "off") {
      let granted = await isPermissionGranted();
      if (!granted) {
        granted = (await requestPermission()) === "granted";
      }
      if (!granted) {
        setPermissionDenied(true);
        updateSetting.mutate({ key: "notification_mode", value: "off" });
        return;
      }
    }
    updateSetting.mutate({ key: "notification_mode", value: mode });
  }

  return (
    <section className="screen">
      <h1>Einstellungen</h1>

      <div className="card">
        <h2>Start</h2>
        <div className="field">
          <label className="switch-label" htmlFor="set-autostart">
            <span className="switch">
              <input
                id="set-autostart"
                type="checkbox"
                checked={autostart.data === true}
                disabled={setAutostart.isPending}
                onChange={(e) => setAutostart.mutate(e.target.checked)}
              />
              <span className="switch-track" aria-hidden="true" />
            </span>
            <span>Mit Windows starten</span>
          </label>
        </div>
        <div className="field">
          <label className="switch-label" htmlFor="set-minimized">
            <span className="switch">
              <input
                id="set-minimized"
                type="checkbox"
                checked={startMinimized}
                onChange={(e) =>
                  updateSetting.mutate({
                    key: "start_minimized",
                    value: e.target.checked ? "1" : "0",
                  })
                }
              />
              <span className="switch-track" aria-hidden="true" />
            </span>
            <span>Minimiert starten</span>
          </label>
          <p className="helper">
            Printy startet dann nur im Infobereich und überwacht die Ordner im
            Hintergrund.
          </p>
        </div>
      </div>

      <div className="card">
        <h2>Benachrichtigungen</h2>
        <div className="modes">
          {NOTIFICATION_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              className={`mode${notificationMode === opt.value ? " on" : ""}`}
              aria-pressed={notificationMode === opt.value}
              onClick={() => void handleNotificationMode(opt.value)}
            >
              {opt.label}
            </button>
          ))}
        </div>
        {permissionDenied && (
          <p className="folder-error" role="alert" style={{ marginTop: "0.75rem" }}>
            Windows erlaubt Printy keine Benachrichtigungen. Bitte in den
            Windows-Einstellungen freigeben.
          </p>
        )}
      </div>

      <div className="card">
        <h2>Erscheinungsbild</h2>
        <div className="modes">
          {THEME_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              className={`mode${theme === opt.value ? " on" : ""}`}
              aria-pressed={theme === opt.value}
              onClick={() => handleTheme(opt.value)}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </div>

      <div className="card">
        <h2>Erweitert</h2>
        <div className="field">
          <label htmlFor="set-sumatra">SumatraPDF (optional)</label>
          <input
            id="set-sumatra"
            className="input"
            value={sumatra}
            placeholder="C:\\Program Files\\SumatraPDF\\SumatraPDF.exe"
            onChange={(e) => {
              sumatraTouched.current = true;
              setSumatra(e.target.value);
            }}
            onBlur={() => updateSetting.mutate({ key: "sumatra_path", value: sumatra })}
          />
          <p className="helper">
            Nur nötig, wenn eine PDF nicht direkt gedruckt werden kann. Printy sucht
            SumatraPDF zuerst an den üblichen Installationsorten. Es wird nicht
            mitgeliefert.
          </p>
        </div>

        <div className="field">
          <label>Datenbank</label>
          <span className="mono muted" data-testid="db-path">
            {dbPath ?? "…"}
          </span>
          <p className="helper">
            <button
              type="button"
              className="link-btn"
              disabled={dbPath === null}
              onClick={() => {
                if (dbPath !== null) void revealItemInDir(dbPath);
              }}
            >
              Im Explorer zeigen
            </button>
          </p>
        </div>
      </div>
    </section>
  );
}
