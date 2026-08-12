import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import Settings from "./Settings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/path", () => ({
  appDataDir: vi.fn(async () => "/Users/tim/Library/Application Support/com.noidee.printy"),
  join: vi.fn(async (...parts: string[]) => parts.join("/")),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: vi.fn(async () => {}),
  openUrl: vi.fn(async () => {}),
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

describe("Settings", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
    notify.isPermissionGranted.mockReset().mockResolvedValue(true);
    notify.requestPermission.mockReset().mockResolvedValue("granted");
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_settings_cmd") {
        return {
          notification_mode: "off",
          autostart: "0",
          start_minimized: "0",
          sumatra_path: "",
          user_paused: "0",
        };
      }
      if (cmd === "get_autostart_cmd") return false;
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

  it("shows where the database lives", async () => {
    renderScreen();
    await waitFor(() =>
      expect(screen.getByTestId("db-path")).toHaveTextContent("printy.sqlite"),
    );
  });
});
