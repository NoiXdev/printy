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

// Verlauf, Einstellungen and Über are not part of this build yet (see the
// task report): Verlauf and Über were cut by explicit deadline decision, and
// Einstellungen shares the not-yet-built Task 9 with Über. Only Ordner has a
// real route, so the navigation currently offers exactly that one entry.
describe("App shell", () => {
  it("renders the brand lockup in the sidebar", () => {
    renderApp();
    expect(screen.getByText("Printy")).toBeInTheDocument();
  });

  it("offers the Ordner navigation entry", () => {
    renderApp();
    const nav = screen.getByRole("navigation", { name: "Hauptnavigation" });
    const links = Array.from(nav.querySelectorAll("a")).map((a) => a.textContent);
    expect(links).toEqual([expect.stringContaining("Ordner")]);
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
