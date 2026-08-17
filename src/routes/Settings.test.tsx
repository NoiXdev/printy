import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import Settings from "./Settings";
import pkg from "../../package.json";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/path", () => ({
  appDataDir: vi.fn(async () => "/Users/tim/Library/Application Support/com.noidee.printy"),
  join: vi.fn(async (...parts: string[]) => parts.join("/")),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: vi.fn(async () => {}),
  openUrl: vi.fn(async () => {}),
}));
const openMock = vi.fn();
const saveMock = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...a: unknown[]) => openMock(...a),
  save: (...a: unknown[]) => saveMock(...a),
}));

// `vi.mock` factories are hoisted above the imports, so the handles they close
// over must be created by `vi.hoisted` rather than by a plain const.
const notify = vi.hoisted(() => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-notification", () => notify);

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

function renderScreen() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <Settings />
    </QueryClientProvider>,
  );
}

/** Per-test overrides consulted before the shared defaults below. A value may
 * be a plain response or a function of the invoke args, for commands (like
 * `folder_delete_impact_cmd`) whose answer depends on which id was asked. */
let extra: Record<string, unknown | ((args?: Record<string, unknown>) => unknown)>;

describe("Settings", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
    notify.isPermissionGranted.mockReset().mockResolvedValue(true);
    notify.requestPermission.mockReset().mockResolvedValue("granted");
    invokeMock.mockReset();
    openMock.mockReset();
    saveMock.mockReset();
    extra = {};
    invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd in extra) {
        const value = extra[cmd];
        return typeof value === "function" ? (value as (a?: typeof args) => unknown)(args) : value;
      }
      if (cmd === "get_settings_cmd") {
        return {
          notification_mode: "off",
          autostart: "0",
          start_minimized: "0",
          sumatra_path: "",
          pdfium_path: "",
          default_poll_interval_secs: "1",
          user_paused: "0",
        };
      }
      if (cmd === "get_autostart_cmd") return false;
      if (cmd === "list_folders_cmd") return [];
      if (cmd === "get_app_version_cmd") return pkg.version;
      return undefined;
    });
  });

  it("asks for permission and stores the mode when the user turns notifications on", async () => {
    notify.isPermissionGranted.mockResolvedValue(false);
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Nur Fehler" }));

    await waitFor(() => expect(notify.requestPermission).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "notification_mode",
        value: "errors",
      }),
    );
  });

  it("falls back to off and says so when permission is refused", async () => {
    notify.isPermissionGranted.mockResolvedValue(false);
    notify.requestPermission.mockResolvedValue("denied");
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Alle" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "notification_mode",
        value: "off",
      }),
    );
    expect(invokeMock).not.toHaveBeenCalledWith("update_setting_cmd", {
      key: "notification_mode",
      value: "all",
    });
    expect(
      screen.getByText(
        "Windows erlaubt Printy keine Benachrichtigungen. Bitte in den Windows-Einstellungen freigeben.",
      ),
    ).toBeInTheDocument();
  });

  it("does not prompt when the user switches notifications off", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Aus" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "notification_mode",
        value: "off",
      }),
    );
    expect(notify.requestPermission).not.toHaveBeenCalled();
  });

  it("routes autostart through the dedicated command, not the settings table", async () => {
    renderScreen();
    fireEvent.click(await screen.findByLabelText("Mit Windows starten"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_autostart_cmd", { enabled: true }),
    );
    expect(invokeMock).not.toHaveBeenCalledWith("update_setting_cmd", {
      key: "autostart",
      value: "1",
    });
  });

  it("writes start-minimised as the flag the backend reads", async () => {
    renderScreen();
    fireEvent.click(await screen.findByLabelText("Minimiert starten"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "start_minimized",
        value: "1",
      }),
    );
  });

  it("keeps the theme in localStorage only and never sends it to the backend", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Dunkel" }));

    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("printy-theme")).toBe("dark");
    expect(
      invokeMock.mock.calls.filter(
        (c) => c[0] === "update_setting_cmd" && (c[1] as { key: string }).key === "theme",
      ),
    ).toEqual([]);
  });

  it("saves the SumatraPDF path on blur", async () => {
    renderScreen();
    const input = await screen.findByLabelText("SumatraPDF (optional)");
    fireEvent.change(input, { target: { value: "C:\\Tools\\SumatraPDF.exe" } });
    fireEvent.blur(input);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "sumatra_path",
        value: "C:\\Tools\\SumatraPDF.exe",
      }),
    );
  });

  it("saves the pdfium path on blur", async () => {
    renderScreen();
    const input = await screen.findByLabelText("pdfium-Bibliothek (optional)");
    fireEvent.change(input, { target: { value: "C:\\Tools\\pdfium.dll" } });
    fireEvent.blur(input);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "pdfium_path",
        value: "C:\\Tools\\pdfium.dll",
      }),
    );
  });

  it("saves the pdfium path immediately once picked from the native dialog", async () => {
    openMock.mockResolvedValue("/Users/tim/pdfium/libpdfium.dylib");
    renderScreen();
    // The pdfium field is the one whose "Durchsuchen …" button sits next to
    // its own labeled input, so scope the click to that field.
    const pdfiumRow = (await screen.findByLabelText("pdfium-Bibliothek (optional)")).closest(
      ".field",
    ) as HTMLElement;
    fireEvent.click(within(pdfiumRow).getByRole("button", { name: "Durchsuchen …" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "pdfium_path",
        value: "/Users/tim/pdfium/libpdfium.dylib",
      }),
    );
  });

  it("saves the default poll interval on blur, floored at 1", async () => {
    renderScreen();
    const input = await screen.findByLabelText(
      "Standard-Prüfintervall für neue Ordner (Sekunden)",
    );
    fireEvent.change(input, { target: { value: "0" } });
    fireEvent.blur(input);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "default_poll_interval_secs",
        value: "1",
      }),
    );
  });

  it("explains the two-scan wait for the default poll interval", async () => {
    renderScreen();
    await waitFor(() =>
      expect(screen.getByTestId("default-interval-hint")).toHaveTextContent(
        "gilt erst als fertig, wenn sie sich zwei Scans lang nicht mehr ändert",
      ),
    );
  });

  it("shows where the database lives", async () => {
    renderScreen();
    await waitFor(() =>
      expect(screen.getByTestId("db-path")).toHaveTextContent("printy.sqlite"),
    );
  });

  // Asserted against the real package.json version, not a literal, so a
  // release bump can never leave this test silently passing while the UI
  // shows a stale number.
  it("shows the running app version as a plain read-only line", async () => {
    renderScreen();
    expect(await screen.findByText(`v${pkg.version}`)).toBeInTheDocument();
  });

  describe("configuration export", () => {
    it("saves the export through the native save dialog and the write command", async () => {
      saveMock.mockResolvedValue("/Users/tim/Downloads/printy-konfiguration.json");
      extra.export_config_cmd = '{"schema_version":1,"folders":[]}';
      renderScreen();

      fireEvent.click(
        await screen.findByRole("button", { name: "Konfiguration exportieren" }),
      );

      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith("write_text_file_cmd", {
          path: "/Users/tim/Downloads/printy-konfiguration.json",
          contents: '{"schema_version":1,"folders":[]}',
        }),
      );

      const [options] = saveMock.mock.calls[0] as [{ defaultPath?: string; filters?: unknown }];
      expect(options.defaultPath).toContain(".json");
      expect(options.filters).toEqual([{ name: "Konfiguration", extensions: ["json"] }]);
    });

    it("does nothing when the save dialog is cancelled", async () => {
      saveMock.mockResolvedValue(null);
      renderScreen();

      fireEvent.click(
        await screen.findByRole("button", { name: "Konfiguration exportieren" }),
      );

      await waitFor(() => expect(saveMock).toHaveBeenCalled());
      expect(invokeMock).not.toHaveBeenCalledWith("export_config_cmd");
      expect(invokeMock).not.toHaveBeenCalledWith(
        "write_text_file_cmd",
        expect.anything(),
      );
    });
  });

  describe("configuration import", () => {
    function pickImportFile(json = '{"schema_version":1}'): void {
      openMock.mockResolvedValue("/Users/tim/Downloads/printy-konfiguration.json");
      extra.read_text_file_cmd = json;
    }

    it("reads the picked file and asks for confirmation before importing anything", async () => {
      pickImportFile();
      renderScreen();

      fireEvent.click(
        await screen.findByRole("button", { name: "Konfiguration importieren" }),
      );

      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith("read_text_file_cmd", {
          path: "/Users/tim/Downloads/printy-konfiguration.json",
        }),
      );
      expect(await screen.findByRole("alertdialog")).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "Hinzufügen und aktualisieren" }),
      ).toHaveAttribute("aria-pressed", "true");
      expect(invokeMock).not.toHaveBeenCalledWith("import_config_cmd", expect.anything());
    });

    it("does nothing when the open dialog is cancelled", async () => {
      openMock.mockResolvedValue(null);
      renderScreen();

      fireEvent.click(
        await screen.findByRole("button", { name: "Konfiguration importieren" }),
      );

      await waitFor(() => expect(openMock).toHaveBeenCalled());
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    });

    it("cancelling the confirmation never calls import_config_cmd", async () => {
      pickImportFile();
      renderScreen();
      fireEvent.click(
        await screen.findByRole("button", { name: "Konfiguration importieren" }),
      );
      await screen.findByRole("alertdialog");

      fireEvent.click(screen.getByRole("button", { name: "Abbrechen" }));

      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
      expect(invokeMock).not.toHaveBeenCalledWith("import_config_cmd", expect.anything());
    });

    it("selecting Alles ersetzen fetches the aggregate impact and shows the destructive warning", async () => {
      pickImportFile();
      extra.list_folders_cmd = [{ id: 1 }, { id: 2 }];
      extra.folder_delete_impact_cmd = (args?: Record<string, unknown>) =>
        args?.id === 1 ? { waiting: 1, history: 2 } : { waiting: 3, history: 4 };
      renderScreen();

      fireEvent.click(
        await screen.findByRole("button", { name: "Konfiguration importieren" }),
      );
      await screen.findByRole("alertdialog");
      fireEvent.click(screen.getByRole("button", { name: "Alles ersetzen" }));

      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith("folder_delete_impact_cmd", { id: 1 }),
      );
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith("folder_delete_impact_cmd", { id: 2 }),
      );
      expect(await screen.findByText(/2 bestehende Ordner/)).toBeInTheDocument();
      expect(screen.getByText(/4 wartende Aufträge/)).toBeInTheDocument();
      expect(screen.getByText(/6 Einträge im Verlauf/)).toBeInTheDocument();
    });

    it("confirms merge import, calls import_config_cmd and shows the disabled folders prominently", async () => {
      pickImportFile('{"schema_version":1,"folders":[]}');
      extra.import_config_cmd = {
        mode: "merge",
        total_folders: 2,
        created: 1,
        updated: 0,
        disabled: 1,
        settings_applied: 4,
        folders: [
          {
            name: "Scans",
            path: "/Users/tim/Scans",
            action: "created",
            enabled: true,
            disabled_reason: null,
          },
          {
            name: "Rechnungen",
            path: "/Users/tim/Rechnungen",
            action: "created",
            enabled: false,
            disabled_reason: { path_missing: true, printer_missing: false },
          },
        ],
      };
      renderScreen();

      fireEvent.click(
        await screen.findByRole("button", { name: "Konfiguration importieren" }),
      );
      await screen.findByRole("alertdialog");
      fireEvent.click(screen.getByRole("button", { name: "Importieren" }));

      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith("import_config_cmd", {
          json: '{"schema_version":1,"folders":[]}',
          mode: "merge",
        }),
      );
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();

      expect(await screen.findByText("Rechnungen")).toBeInTheDocument();
      expect(
        screen.getByText("Ordnerpfad auf diesem Rechner nicht gefunden."),
      ).toBeInTheDocument();
      expect(screen.queryByText("Scans")).not.toBeInTheDocument();
    });
  });
});
