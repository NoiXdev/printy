import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import About from "./About";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => {}) }));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;
const openUrlMock = openUrl as unknown as ReturnType<typeof vi.fn>;

function renderScreen() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <About />
    </QueryClientProvider>,
  );
}

describe("About", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openUrlMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) =>
      cmd === "get_app_version_cmd" ? "1.2.3" : undefined,
    );
  });

  // The version comes from the running binary, not from a copy in the
  // frontend, so it cannot drift from what the release workflow stamped in.
  it("shows the version reported by the running binary", async () => {
    renderScreen();
    expect(await screen.findByText("Version 1.2.3")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("get_app_version_cmd");
  });

  it("opens links in the system browser rather than in the app window", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Dokumentation" }));
    await waitFor(() =>
      expect(openUrlMock).toHaveBeenCalledWith("https://docs.noix.dev/printy/"),
    );
  });

  it("lists third-party licences and filters them by name", async () => {
    renderScreen();
    const filter = await screen.findByLabelText("Lizenzen nach Name filtern");

    const all = screen.getByTestId("licence-count").textContent ?? "";
    expect(all).toMatch(/^\d+ von \d+ Einträgen$/);

    fireEvent.change(filter, { target: { value: "react-dom" } });
    await waitFor(() =>
      expect(screen.getByTestId("licence-count")).not.toHaveTextContent(all),
    );
    expect(screen.getByTestId("licence-list").querySelectorAll("li").length).toBeGreaterThan(
      0,
    );
  });

  it("says so when a filter matches nothing instead of showing an empty list", async () => {
    renderScreen();
    const filter = await screen.findByLabelText("Lizenzen nach Name filtern");
    fireEvent.change(filter, { target: { value: "definitiv-kein-paket-xyz" } });
    expect(await screen.findByText("Kein Treffer")).toBeInTheDocument();
  });
});
