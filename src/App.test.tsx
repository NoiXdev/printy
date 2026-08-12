import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import App from "./App";

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

function renderApp() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );
}

// Verlauf and Über are cut from this build by explicit deadline decision and
// must not point at nonexistent components. Einstellungen now has a real
// screen (Task 9), so the navigation offers exactly Ordner and Einstellungen.
describe("App shell", () => {
  it("renders the brand lockup in the sidebar", () => {
    renderApp();
    expect(screen.getByText("Printy")).toBeInTheDocument();
  });

  it("offers the Ordner and Einstellungen navigation entries", () => {
    renderApp();
    const nav = screen.getByRole("navigation", { name: "Hauptnavigation" });
    const links = Array.from(nav.querySelectorAll("a")).map((a) => a.textContent);
    expect(links).toEqual([
      expect.stringContaining("Ordner"),
      expect.stringContaining("Einstellungen"),
    ]);
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
});
