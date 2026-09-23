import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import pkg from "../package.json";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => []) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/path", () => ({
  appDataDir: vi.fn(async () => "/tmp"),
  join: vi.fn(async (...p: string[]) => p.join("/")),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: vi.fn(async () => {}),
  openUrl: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(async () => true),
  requestPermission: vi.fn(async () => "granted"),
  sendNotification: vi.fn(),
}));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

function renderApp() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );
}

describe("App shell", () => {
  it("renders the brand lockup in the sidebar", () => {
    renderApp();
    expect(screen.getByText("Printy")).toBeInTheDocument();
  });

  it("offers every screen in the navigation", () => {
    renderApp();
    const nav = screen.getByRole("navigation", { name: "Hauptnavigation" });
    const links = Array.from(nav.querySelectorAll("a")).map((a) => a.textContent);
    expect(links).toEqual([
      expect.stringContaining("Ordner"),
      expect.stringContaining("Verlauf"),
      expect.stringContaining("Einstellungen"),
      expect.stringContaining("Über"),
    ]);
  });

  // Every navigation entry must resolve to a route. A link to a path the
  // router does not know renders an empty <main>, which looks like a crash.
  it("routes every navigation entry to a screen", () => {
    renderApp();
    const nav = screen.getByRole("navigation", { name: "Hauptnavigation" });
    const paths = Array.from(nav.querySelectorAll("a")).map((a) =>
      a.getAttribute("href"),
    );
    expect(paths).toEqual(["/", "/verlauf", "/einstellungen", "/ueber"]);
  });

  it("marks Ordner as the active start route", () => {
    renderApp();
    const nav = screen.getByRole("navigation", { name: "Hauptnavigation" });
    const active = nav.querySelector("a.active");
    expect(active).not.toBeNull();
    expect(active).toHaveTextContent("Ordner");
  });

  it("renders the Ordner screen as the index route", () => {
    renderApp();
    expect(screen.getByRole("heading", { level: 1, name: "Ordner" })).toBeInTheDocument();
  });

  // Asserted against the real package.json version, not a literal, so a
  // release bump can never leave this test silently passing while the UI
  // shows a stale number.
  it("shows the running app version in the sidebar footer, prefixed with v", async () => {
    invokeMock.mockImplementation(async (cmd: string) =>
      cmd === "get_app_version_cmd" ? pkg.version : [],
    );
    renderApp();
    expect(await screen.findByText(`v${pkg.version}`)).toBeInTheDocument();
  });
});
