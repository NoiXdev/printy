# Printy UI and Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Printy's user interface, brand identity and shell behaviour — the sidebar app with the Ordner, Verlauf, Einstellungen and Über screens, a state-coloured tray icon, close-to-tray, autostart and desktop notifications — on top of the Tauri command surface delivered by plan 1.

**Architecture:** A React 19 + TypeScript SPA rendered inside the existing Tauri 2 window, using React Router for the four screens and TanStack Query for every backend read. A single typed `api` object wraps `invoke`, so no component calls Tauri directly; backend pushes arrive through three `listen` helpers for `printy://job`, `printy://folder` and `printy://queue`. Tray, close-to-tray and autostart are Rust additions that extend plan 1's `src-tauri/src/lib.rs` in a new `shell` module.

**Tech Stack:** React 19, TypeScript 5.8, Vite 7, React Router 7, TanStack Query 5, `@tauri-apps/api` 2, `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-opener`, `@tauri-apps/plugin-notification`, `@fontsource-variable/fraunces`, Vitest 4 + Testing Library, Rust 2021 / Tauri 2 (`tray-icon` feature, `tauri-plugin-autostart`).

## Global Constraints

- Reference spec: `docs/superpowers/specs/2026-08-12-printy-design.md`, sections 4, 10, 11 and 13. It is authoritative; where this plan and the spec disagree, stop and ask.
- Backend contract: `docs/superpowers/plans/2026-08-12-printy-core-engine.md`, Task 17. Command names, parameter names and return shapes are copied from it verbatim. Never invent a command.
- Conventions are inherited from the sibling project `/Users/noidee/_dev/tabs-manager` (tabsy). Match it rather than inventing new patterns.
- Code, identifiers, comments, file names and commit messages are **English**. Every user-visible string is **German**.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `test:`, `chore:`, `refactor:`, `build:`, `docs:`).
- Colour roles are fixed: coral `#ff7a59` is the lead accent (primary buttons, active nav, brand); mint `#2fe6b7` marks success and running state **only**; teal `#0a2826` is the sidebar and ink; cream `#f4f1ea` is the page; `#fffdf8` is a card. Never use mint for a neutral accent and never use coral for a success state.
- Radii are `--radius: 16px` and `--radius-sm: 12px`. Display font is Fraunces Variable, self-hosted through `@fontsource-variable/fraunces`.
- New npm dependencies are limited to what tabsy already uses, plus `@fontsource-variable/fraunces` and `@tauri-apps/plugin-dialog` (the JS binding for the Rust plugin plan 1 already installed and already granted `dialog:allow-open`).
- Every component receives data through props or the `api` object. No component imports `invoke` directly.
- Tests are Vitest + Testing Library, colocated as `*.test.tsx` / `*.test.ts` next to the file under test, opening with explicit `import { describe, it, expect, vi } from "vitest";` as in `tabs-manager/src/components/SearchSelect.test.tsx`. Tauri is never reachable in jsdom, so every test file that transitively imports `@tauri-apps/api/core` mocks it with `vi.mock`.
- Rust additions follow plan 1's rules: commands are suffixed `_cmd`, take `state: State<'_, AppState>` first, and are thin wrappers over testable functions. No new Rust dev-dependencies.
- Every task ends with a green test run and a commit.

---

### Task 1: Brand foundation — tokens, fonts, theme switch, shared screen styles

**Files:**
- Create: `src/theme.css`
- Create: `src/routes/screens.css`
- Create: `src/lib/theme.ts`
- Create: `vitest.config.ts`
- Test: `src/lib/theme.test.ts`
- Modify: `package.json` (dependencies and the `test` script)
- Modify: `index.html` (document title)
- Modify: `src-tauri/tauri.conf.json` (`productName`, window title)

**Interfaces:**
- Consumes: nothing.
- Produces: the CSS custom properties `--coral`, `--mint`, `--teal`, `--cream`, `--surface`, `--ink`, `--radius`, `--radius-sm`, `--font-display`; `lib/theme::{ThemeChoice, getThemeChoice, resolveTheme, applyTheme, initTheme}`; the shared class names `.screen`, `.card`, `.field`, `.input`, `.btn`, `.badge`, `.switch`, `.modal`, `.folder-card`, `.status-dot`, `.history-row`.

- [ ] **Step 1: Install the dependencies**

Run in the repository root:

```bash
npm install @tanstack/react-query@^5.101.2 react-router-dom@^7.18.0 \
  @fontsource-variable/fraunces@^5.2.9 @tauri-apps/plugin-notification@^2.3.3 \
  @tauri-apps/plugin-dialog@^2
npm install -D vitest@^4.1.9 jsdom@^29.1.1 @testing-library/react@^16.3.2 \
  @testing-library/jest-dom@^6.9.1
```

Then set the package identity and test script in `package.json`:

```json
{
  "name": "printy",
  "private": true,
  "version": "0.1.0",
  "license": "MIT",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "test": "vitest run"
  }
}
```

Create `vitest.config.ts`, identical to tabsy's:

```ts
import { defineConfig } from "vitest/config";
export default defineConfig({
  test: { environment: "jsdom", globals: true },
});
```

- [ ] **Step 2: Write the failing test**

Create `src/lib/theme.test.ts`:

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { applyTheme, getThemeChoice, resolveTheme } from "./theme";

describe("theme", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
  });

  it("defaults to system when nothing is stored", () => {
    expect(getThemeChoice()).toBe("system");
  });

  it("resolves an explicit choice without consulting the media query", () => {
    expect(resolveTheme("dark")).toBe("dark");
    expect(resolveTheme("light")).toBe("light");
  });

  it("resolves system from prefers-color-scheme", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({ matches: true, addEventListener: vi.fn() })),
    );
    expect(resolveTheme("system")).toBe("dark");
    vi.unstubAllGlobals();
  });

  it("applies the resolved theme to the root element and persists the choice", () => {
    applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(getThemeChoice()).toBe("dark");
  });

  it("rejects a corrupted stored value and falls back to system", () => {
    localStorage.setItem("printy-theme", "banana");
    expect(getThemeChoice()).toBe("system");
  });
});
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `npx vitest run src/lib/theme.test.ts`
Expected: FAIL — `Failed to resolve import "./theme"`, the module does not exist yet.

- [ ] **Step 4: Write the theme module**

Create `src/lib/theme.ts`:

```ts
export type ThemeChoice = "light" | "dark" | "system";

const KEY = "printy-theme";

function isThemeChoice(value: string | null): value is ThemeChoice {
  return value === "light" || value === "dark" || value === "system";
}

export function getThemeChoice(): ThemeChoice {
  try {
    const stored = localStorage.getItem(KEY);
    if (isThemeChoice(stored)) return stored;
  } catch {
    // localStorage may be unavailable; fall through to default.
  }
  return "system";
}

export function resolveTheme(choice: ThemeChoice): "light" | "dark" {
  if (choice === "light" || choice === "dark") return choice;

  try {
    if (
      typeof window !== "undefined" &&
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches
    ) {
      return "dark";
    }
  } catch {
    // matchMedia may be unavailable; default to light.
  }
  return "light";
}

export function applyTheme(choice: ThemeChoice): void {
  document.documentElement.dataset.theme = resolveTheme(choice);
  try {
    localStorage.setItem(KEY, choice);
  } catch {
    // Persisting is best-effort.
  }
}

export function initTheme(): void {
  applyTheme(getThemeChoice());

  try {
    if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
      const media = window.matchMedia("(prefers-color-scheme: dark)");
      media.addEventListener("change", () => {
        if (getThemeChoice() === "system") {
          applyTheme("system");
        }
      });
    }
  } catch {
    // Listener is best-effort.
  }
}
```

- [ ] **Step 5: Write the design tokens**

Create `src/theme.css`. The palette is tabsy's with Printy's accent roles: coral leads, mint is reserved for success and running.

```css
/* Printy design tokens.
   Display face is Fraunces Variable, self-hosted via
   @fontsource-variable/fraunces (imported in main.tsx); no network call. */

:root {
  /* Brand palette */
  --coral: #ff7a59;
  --mint: #2fe6b7;
  --teal: #0a2826;
  --cream: #f4f1ea;
  --teal-soft: #0a8f73;
  --ink: #0a2826;

  /* Derived tints */
  --coral-soft: rgba(255, 122, 89, 0.14);
  --coral-line: rgba(255, 122, 89, 0.35);
  --mint-soft: rgba(47, 230, 183, 0.14);
  --mint-line: rgba(47, 230, 183, 0.35);
  --ink-12: rgba(10, 40, 38, 0.12);
  --ink-08: rgba(10, 40, 38, 0.08);
  --ink-60: rgba(10, 40, 38, 0.6);
  --ink-45: rgba(10, 40, 38, 0.45);
  --surface: #fffdf8;
  --teal-grad: linear-gradient(150deg, #0f3d3a, #0a2826);
  /* Neutral state colour: paused folders and an idle tray. */
  --grey: #9aa3a1;

  /* Type */
  --font: -apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif;
  --font-display: "Fraunces Variable", "Iowan Old Style", "Palatino", Georgia, serif;
  --font-mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;

  /* Shape & depth */
  --radius: 16px;
  --radius-sm: 12px;
  --radius-pill: 999px;
  --shadow-sm: 0 1px 2px rgba(10, 40, 38, 0.06), 0 1px 3px rgba(10, 40, 38, 0.05);
  --shadow-md: 0 6px 22px rgba(10, 40, 38, 0.09);
  --shadow-coral: 0 10px 24px rgba(255, 122, 89, 0.32);
  --ring: 0 0 0 3px var(--coral-soft);
}

/* Dark theme: keep the coral/mint accents and the teal sidebar, recolour
   surfaces and ink for a dark teal-black palette. */
:root[data-theme="dark"] {
  --cream: #0d1716;
  --ink: #eef3f1;
  --surface: #14211f;
  --grey: #6d7a78;

  --ink-12: rgba(238, 243, 241, 0.12);
  --ink-08: rgba(238, 243, 241, 0.08);
  --ink-60: rgba(238, 243, 241, 0.62);
  --ink-45: rgba(238, 243, 241, 0.42);

  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.3), 0 1px 3px rgba(0, 0, 0, 0.25);
  --shadow-md: 0 6px 22px rgba(0, 0, 0, 0.4);
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  font-family: var(--font);
  font-size: 15px;
  line-height: 1.5;
  background: var(--cream);
  color: var(--ink);
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}

/* Atmospheric backdrop: faint warm washes so the cream surface gains depth.
   Coral leads, mint only whispers. */
body::before {
  content: "";
  position: fixed;
  inset: 0;
  z-index: -1;
  background:
    radial-gradient(55% 45% at 14% -8%, rgba(255, 122, 89, 0.1), transparent 70%),
    radial-gradient(45% 40% at 102% -2%, rgba(47, 230, 183, 0.06), transparent 70%);
  pointer-events: none;
}

:root[data-theme="dark"] body::before {
  background:
    radial-gradient(55% 45% at 14% -8%, rgba(255, 122, 89, 0.07), transparent 70%),
    radial-gradient(45% 40% at 102% -2%, rgba(47, 230, 183, 0.04), transparent 70%);
}

::selection {
  background: var(--coral);
  color: #fff;
}

:focus-visible {
  outline: none;
  box-shadow: var(--ring);
  border-radius: 8px;
}
```

- [ ] **Step 6: Write the shared screen styles**

Create `src/routes/screens.css`. Everything every screen in this plan needs lives here, so no later task edits CSS.

```css
/* ============================================================
   Printy — shared screen styles
   Visual layer only. No logic, payload, or routing changes.
   ============================================================ */

.screen {
  max-width: 760px;
  animation: printy-rise 0.4s cubic-bezier(0.22, 1, 0.36, 1) both;
}

@keyframes printy-rise {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: none; }
}

/* ---- Typographic scale -------------------------------------------------- */
.screen h1 {
  margin: 0 0 1.5rem;
  font-family: var(--font-display);
  font-size: 2rem;
  font-weight: 600;
  letter-spacing: -0.015em;
  line-height: 1.1;
}
.card h2 {
  margin: 0 0 1rem;
  font-family: var(--font-display);
  font-size: 1.2rem;
  font-weight: 600;
  letter-spacing: -0.01em;
}
.section-title {
  margin: 1.6rem 0 0.75rem;
  font-family: var(--font-display);
  font-size: 1.1rem;
  color: var(--ink);
}
.muted { color: var(--ink-60); }
.helper {
  font-size: 0.82rem;
  color: var(--ink-60);
  line-height: 1.5;
  margin: 0.75rem 0 0;
}
.mono {
  font-family: var(--font-mono);
  font-size: 0.8rem;
  word-break: break-all;
}

/* ---- Cards -------------------------------------------------------------- */
.card {
  background: var(--surface);
  border: 1px solid var(--ink-08);
  border-radius: var(--radius);
  padding: 1.4rem 1.5rem;
  box-shadow: var(--shadow-md);
}
.card + .card { margin-top: 1rem; }

/* ---- Forms -------------------------------------------------------------- */
.field {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  margin-bottom: 1rem;
}
.field label {
  font-size: 0.74rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--ink-45);
}
.input {
  font: inherit;
  padding: 0.62rem 0.8rem;
  border: 1.5px solid var(--ink-12);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--ink);
  outline: none;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}
.input::placeholder { color: var(--ink-45); }
.input:hover:not(:disabled) { border-color: var(--ink-45); }
.input:focus { border-color: var(--coral); box-shadow: var(--ring); }
.input:disabled { opacity: 0.5; cursor: not-allowed; }
.row { display: flex; gap: 0.75rem; align-items: flex-end; }
.row .field { margin-bottom: 0; }

/* ---- Buttons ------------------------------------------------------------ */
.btn {
  font: inherit;
  font-weight: 700;
  padding: 0.62rem 1.25rem;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  background: var(--coral);
  color: #fff;
  box-shadow: 0 1px 2px rgba(255, 122, 89, 0.4);
  transition: transform 0.12s ease, box-shadow 0.16s ease, background 0.16s ease,
    opacity 0.16s ease;
}
.btn:hover:not(:disabled) {
  background: #ff8c6f;
  box-shadow: var(--shadow-coral);
  transform: translateY(-1px);
}
.btn:active:not(:disabled) {
  transform: translateY(0);
  box-shadow: 0 2px 6px rgba(255, 122, 89, 0.3);
}
.btn:disabled { opacity: 0.45; cursor: not-allowed; box-shadow: none; }
.btn-quiet {
  background: transparent;
  color: var(--ink);
  border: 1.5px solid var(--ink-12);
  box-shadow: none;
}
.btn-quiet:hover:not(:disabled) {
  background: var(--ink-08);
  color: var(--ink);
  box-shadow: none;
  transform: translateY(-1px);
}
.link-btn {
  background: none;
  border: none;
  padding: 0;
  font: inherit;
  font-weight: 600;
  color: var(--teal-soft);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;
  transition: color 0.15s ease;
}
.link-btn:hover { color: var(--coral); }

/* ---- Badges, chips and dots --------------------------------------------- */
.badge {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.78rem;
  font-weight: 700;
  padding: 0.32rem 0.7rem;
  border-radius: var(--radius-pill);
  letter-spacing: 0.01em;
  background: var(--ink-08);
  color: var(--ink-60);
}
.badge.on { background: var(--mint-soft); color: var(--teal-soft); }
.badge.off { background: var(--coral-soft); color: var(--coral); }

.chip {
  font: inherit;
  font-weight: 600;
  font-size: 0.85rem;
  padding: 0.32rem 0.85rem;
  border-radius: var(--radius-pill);
  border: 2px solid var(--ink-12);
  background: var(--surface);
  color: var(--ink);
  cursor: pointer;
  opacity: 0.65;
  transition: opacity 0.15s ease, border-color 0.15s ease, background 0.15s ease;
}
.chip:hover:not(:disabled) { opacity: 0.9; }
.chip.on {
  opacity: 1;
  border-color: var(--coral);
  background: var(--coral-soft);
}
.chip:disabled { opacity: 0.35; cursor: not-allowed; }
.chip-row { display: flex; flex-wrap: wrap; gap: 0.5rem; }

.status-dot {
  width: 0.62rem;
  height: 0.62rem;
  flex-shrink: 0;
  border-radius: var(--radius-pill);
  background: var(--grey);
}
.status-dot.running { background: var(--mint); }
.status-dot.error { background: var(--coral); }
.status-dot.paused { background: var(--grey); }
.status-dot.waiting { background: var(--grey); }

/* ---- Switch ------------------------------------------------------------- */
.switch-label {
  display: inline-flex;
  align-items: center;
  gap: 0.6rem;
  font-weight: 600;
  cursor: pointer;
}
.switch { position: relative; display: inline-flex; flex-shrink: 0; }
.switch input {
  position: absolute;
  opacity: 0;
  width: 100%;
  height: 100%;
  margin: 0;
  cursor: pointer;
}
.switch-track {
  width: 2.4rem;
  height: 1.4rem;
  border-radius: var(--radius-pill);
  background: var(--ink-12);
  transition: background 0.16s ease;
  position: relative;
}
.switch-track::after {
  content: "";
  position: absolute;
  top: 0.18rem;
  left: 0.18rem;
  width: 1.04rem;
  height: 1.04rem;
  border-radius: var(--radius-pill);
  background: #fff;
  box-shadow: var(--shadow-sm);
  transition: transform 0.16s ease;
}
.switch input:checked + .switch-track { background: var(--coral); }
.switch input:checked + .switch-track::after { transform: translateX(1rem); }
.switch input:focus-visible + .switch-track { box-shadow: var(--ring); }
.switch input:disabled + .switch-track { opacity: 0.5; }

/* ---- Mode segmented control -------------------------------------------- */
.modes {
  display: flex;
  gap: 0.4rem;
  background: var(--ink-08);
  padding: 0.3rem;
  border-radius: var(--radius-sm);
}
.mode {
  flex: 1;
  font: inherit;
  font-weight: 600;
  font-size: 0.9rem;
  padding: 0.55rem;
  border-radius: 9px;
  border: none;
  background: transparent;
  color: var(--ink-60);
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease, box-shadow 0.16s ease;
}
.mode:hover:not(.on) { color: var(--coral); }
.mode.on { background: var(--coral); color: #fff; box-shadow: var(--shadow-sm); }

/* ---- Screen header and aggregate --------------------------------------- */
.screen-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  margin-bottom: 0.75rem;
}
.screen-header h1 { margin: 0; }
.aggregate {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
  margin-bottom: 1.4rem;
  color: var(--ink-60);
  font-weight: 600;
}

/* ---- Folder cards ------------------------------------------------------- */
.folder-grid {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
.folder-card {
  background: var(--surface);
  border: 1px solid var(--ink-08);
  border-radius: var(--radius);
  padding: 1rem 1.15rem;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  transition: transform 0.14s ease, box-shadow 0.16s ease;
}
.folder-card:hover { transform: translateY(-1px); box-shadow: var(--shadow-md); }
.folder-card.has-error { border-color: var(--coral-line); }
.folder-head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}
.folder-name { font-weight: 700; font-size: 1.02rem; }
.folder-state { font-size: 0.82rem; color: var(--ink-60); font-weight: 600; }
.folder-summary { font-size: 0.85rem; color: var(--ink-60); }
.folder-meta {
  display: flex;
  gap: 1rem;
  flex-wrap: wrap;
  font-size: 0.8rem;
  color: var(--ink-45);
}
.folder-error {
  font-size: 0.85rem;
  color: var(--coral);
  font-weight: 600;
}
.folder-actions {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
  padding-top: 0.35rem;
  border-top: 1px solid var(--ink-08);
}
.add-folder-tile {
  width: 100%;
  font: inherit;
  font-weight: 700;
  color: var(--ink-60);
  padding: 1.4rem;
  border: 2px dashed var(--ink-12);
  border-radius: var(--radius);
  background: transparent;
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease, background 0.15s ease;
}
.add-folder-tile:hover {
  border-color: var(--coral);
  color: var(--coral);
  background: var(--coral-soft);
}

/* ---- History ------------------------------------------------------------ */
.history-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.history-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
  background: var(--surface);
  border: 1px solid var(--ink-08);
  border-radius: var(--radius-sm);
  padding: 0.7rem 0.9rem;
}
.history-main { display: flex; flex-direction: column; gap: 0.2rem; min-width: 0; }
.history-file { font-weight: 600; }
.history-meta { font-size: 0.8rem; color: var(--ink-60); }
.history-right { display: flex; align-items: center; gap: 0.7rem; flex-shrink: 0; }
.history-time {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  font-size: 0.8rem;
  color: var(--ink-45);
}

/* ---- Modal -------------------------------------------------------------- */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(10, 40, 38, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
  z-index: 100;
}
.modal {
  width: 100%;
  max-width: 620px;
  max-height: 86vh;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  background: var(--surface);
  border: 1px solid var(--ink-08);
  border-radius: var(--radius);
  box-shadow: var(--shadow-md);
  padding: 1.25rem 1.4rem;
}
.modal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}
.modal-head h2 { margin: 0; font-family: var(--font-display); }
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  align-items: center;
  margin-top: 0.5rem;
}

/* ---- Empty and loading -------------------------------------------------- */
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 0.35rem;
  padding: 2.4rem 1.5rem;
}
.empty-title { margin: 0; font-family: var(--font-display); font-size: 1.15rem; }
.empty-sub { margin: 0; font-size: 0.9rem; color: var(--ink-60); }
.loading { display: flex; align-items: center; gap: 0.6rem; color: var(--ink-60); }
.spinner {
  width: 1.05rem;
  height: 1.05rem;
  border-radius: var(--radius-pill);
  border: 2px solid var(--ink-12);
  border-top-color: var(--coral);
  animation: printy-spin 0.8s linear infinite;
}
@keyframes printy-spin { to { transform: rotate(360deg); } }
```

- [ ] **Step 7: Set the product name**

Replace the `<title>` element in `index.html` with:

```html
    <title>Printy</title>
```

In `src-tauri/tauri.conf.json`, set `productName` and the window title:

```json
  "productName": "Printy",
```

```json
      {
        "title": "Printy",
        "width": 900,
        "height": 680
      }
```

- [ ] **Step 8: Run the test to verify it passes**

Run: `npx vitest run src/lib/theme.test.ts`
Expected: PASS — 5 tests.

- [ ] **Step 9: Commit**

```bash
git add package.json package-lock.json vitest.config.ts index.html src/theme.css \
  src/routes/screens.css src/lib/theme.ts src/lib/theme.test.ts src-tauri/tauri.conf.json
git commit -m "feat: add printy design tokens, shared screen styles and theme switch"
```

---

### Task 2: Logo component and Windows icon set

**Files:**
- Create: `src/components/Logo.tsx`
- Create: `assets/logo/printy-logo.svg`
- Create: `scripts/gen-icons.sh`
- Test: `src/components/Logo.test.tsx`
- Modify: `src-tauri/tauri.conf.json` (bundle icon list)

**Interfaces:**
- Consumes: the CSS custom properties `--coral`, `--mint`, `--teal` from Task 1.
- Produces: `components/Logo` — `default function Logo({ size?: number; wordmark?: boolean; wordmarkColor?: string; className?: string }): JSX.Element`; the generated icon set under `src-tauri/icons/`.

The mark is a teal folder with a coral sheet emerging and a mint confirmation badge. The folder body carries a literal `#f4f1ea` rim so the teal shape stays legible both on the teal sidebar and on the dark-theme page — tabsy uses the same literal for its badge stroke.

- [ ] **Step 1: Write the failing test**

Create `src/components/Logo.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import "@testing-library/jest-dom";
import Logo from "./Logo";

describe("Logo", () => {
  it("renders the mark at the requested size", () => {
    const { container } = render(<Logo size={48} />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg).toHaveAttribute("width", "48");
    expect(svg).toHaveAttribute("height", "48");
    expect(svg).toHaveAttribute("viewBox", "0 0 48 48");
  });

  it("hides the bare mark from assistive technology", () => {
    const { container } = render(<Logo />);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("renders the wordmark only when asked", () => {
    const { rerender } = render(<Logo />);
    expect(screen.queryByText("Printy")).not.toBeInTheDocument();
    rerender(<Logo wordmark />);
    expect(screen.getByText("Printy")).toBeInTheDocument();
  });

  it("colours the wordmark from the prop", () => {
    render(<Logo wordmark wordmarkColor="#f4f1ea" />);
    expect(screen.getByText("Printy")).toHaveStyle({ color: "#f4f1ea" });
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/components/Logo.test.tsx`
Expected: FAIL — `Failed to resolve import "./Logo"`, the component does not exist yet.

- [ ] **Step 3: Write the component**

Create `src/components/Logo.tsx`:

```tsx
import type { JSX } from "react";

interface LogoProps {
  /** Pixel size of the square mark. Defaults to 28. */
  size?: number;
  /** Render the "Printy" wordmark next to the mark. Defaults to false. */
  wordmark?: boolean;
  /** Color of the wordmark text. Defaults to currentColor (inherits). */
  wordmarkColor?: string;
  className?: string;
}

/**
 * The Printy brand mark: a teal folder with a coral sheet emerging from it and
 * a mint confirmation badge. Rendered as crisp inline SVG so it scales to any
 * size. The folder carries a literal cream rim rather than a themed one, so it
 * stays legible on the teal sidebar and on the dark-theme page alike. Purely
 * presentational — `aria-hidden` on the mark; pass a wordmark for a labelled
 * lockup.
 */
export default function Logo({
  size = 28,
  wordmark = false,
  wordmarkColor = "currentColor",
  className,
}: LogoProps): JSX.Element {
  const mark = (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      fill="none"
      aria-hidden="true"
      role="img"
    >
      {/* coral sheet emerging from the folder, tilted out of the pocket */}
      <rect
        x="18"
        y="4"
        width="16"
        height="20"
        rx="2"
        fill="var(--coral)"
        transform="rotate(7 26 14)"
      />
      <rect
        x="21"
        y="10"
        width="9"
        height="2"
        rx="1"
        fill="#fff5f2"
        transform="rotate(7 26 14)"
      />
      <rect
        x="21"
        y="15"
        width="6"
        height="2"
        rx="1"
        fill="#fff5f2"
        transform="rotate(7 26 14)"
      />
      {/* teal folder body, in front of the sheet */}
      <path
        d="M4 16h11l3 4h23a3 3 0 0 1 3 3v17a3 3 0 0 1-3 3H4a3 3 0 0 1-3-3V19a3 3 0 0 1 3-3z"
        fill="var(--teal)"
        stroke="#f4f1ea"
        strokeWidth="1.5"
      />
      {/* mint confirmation badge */}
      <circle cx="36" cy="35" r="6" fill="var(--mint)" />
      <path
        d="M33.4 35l2 2 3-3.6"
        stroke="var(--teal)"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );

  if (!wordmark) return className ? <span className={className}>{mark}</span> : mark;

  return (
    <span
      className={className}
      style={{ display: "inline-flex", alignItems: "center", gap: size * 0.32 }}
    >
      {mark}
      <span
        style={{
          fontFamily: "var(--font-display)",
          fontWeight: 800,
          fontSize: size * 0.72,
          letterSpacing: "-0.01em",
          color: wordmarkColor,
          lineHeight: 1,
        }}
      >
        Printy
      </span>
    </span>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/components/Logo.test.tsx`
Expected: PASS — 4 tests.

- [ ] **Step 5: Write the standalone icon artwork**

Create `assets/logo/printy-logo.svg`. This is the app-icon lockup, not the in-app mark: it has literal colours (no CSS custom properties, because the rasteriser has no page context) and a cream rounded plate so the icon reads on any desktop background.

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <rect width="1024" height="1024" rx="224" fill="#f4f1ea"/>
  <g transform="translate(160 160) scale(14.6667)">
    <rect x="18" y="4" width="16" height="20" rx="2" fill="#ff7a59" transform="rotate(7 26 14)"/>
    <rect x="21" y="10" width="9" height="2" rx="1" fill="#fff5f2" transform="rotate(7 26 14)"/>
    <rect x="21" y="15" width="6" height="2" rx="1" fill="#fff5f2" transform="rotate(7 26 14)"/>
    <path d="M4 16h11l3 4h23a3 3 0 0 1 3 3v17a3 3 0 0 1-3 3H4a3 3 0 0 1-3-3V19a3 3 0 0 1 3-3z" fill="#0a2826"/>
    <circle cx="36" cy="35" r="6" fill="#2fe6b7"/>
    <path d="M33.4 35l2 2 3-3.6" stroke="#0a2826" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
  </g>
</svg>
```

- [ ] **Step 6: Write the icon generation script**

Create `scripts/gen-icons.sh`. It rasterises the SVG to a 1024px PNG through a one-shot `npx` binary — the same "no project dependency, fetched on demand" approach tabsy uses in `scripts/gen-licenses.sh` — and then hands that PNG to the Tauri icon pipeline, which emits `icon.ico` with every Windows size, `icon.icns`, the PNG ladder and the Square*Logo set.

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

SRC="assets/logo/printy-logo.svg"
PNG="assets/logo/printy-logo-1024.png"

# Rasterise the vector master. resvg is fetched on demand, never installed as a
# project dependency.
npx --yes @resvg/resvg-js-cli "$SRC" "$PNG" --width 1024 --height 1024

# Emits src-tauri/icons/: icon.ico (16/24/32/48/64/256), icon.icns, the PNG
# ladder and the Windows Store Square*Logo set.
npm run tauri -- icon "$PNG"

echo "icons regenerated from $SRC"
```

Then run:

```bash
chmod +x scripts/gen-icons.sh
./scripts/gen-icons.sh
```

- [ ] **Step 7: Point the bundle at the generated icons**

In `src-tauri/tauri.conf.json`, set the `bundle.icon` array to the files the pipeline produced:

```json
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
```

Verify the Windows icon really carries every size:

Run: `ls -l src-tauri/icons/icon.ico src-tauri/icons/icon.icns src-tauri/icons/32x32.png src-tauri/icons/128x128.png`
Expected: all four exist and none is zero bytes.

Run: `python3 -c "import struct;d=open('src-tauri/icons/icon.ico','rb').read();n=struct.unpack('<H',d[4:6])[0];print(n,[d[6+i*16] or 256 for i in range(n)])"`
Expected: at least five entries, including a 16, a 32, a 48 and a 256 pixel image.

- [ ] **Step 8: Commit**

```bash
git add src/components/Logo.tsx src/components/Logo.test.tsx assets/logo \
  scripts/gen-icons.sh src-tauri/icons src-tauri/tauri.conf.json
git commit -m "feat: add printy logo component and generated windows icon set"
```

---

### Task 3: Rust command additions the UI shell needs

**Files:**
- Create: `src-tauri/src/commands/shell.rs`
- Modify: `src-tauri/src/commands/mod.rs` (add `pub mod shell;`)
- Modify: `src-tauri/src/lib.rs` (three entries in the `tauri::generate_handler!` list added by plan 1's Task 17, Step 5)

**Interfaces:**
- Consumes: `intake::scan::scan_folder(root: &Path, types: &[String]) -> std::io::Result<Vec<PathBuf>>` (plan 1, Task 10); `db::settings::set_setting`; `error::{AppError, AppResult}`; `AppState`.
- Produces: `commands::shell::{count_existing, count_existing_files_cmd, get_autostart_cmd, set_autostart_cmd}`.

Plan 1's Task 17 deliberately stops at the fourteen commands the spec's section 11 lists. Three gaps remain that only the UI can see, and all three are additions rather than changes:

1. The create dialog must say *"vorhandene 23 Dateien jetzt mitdrucken"*. The count needs a directory listing that honours the folder's type filter and skips `printed/` and `failed/`, which is exactly `scan_folder`.
2. Plan 1 persists the `autostart` setting but nothing applies it to the operating system. Toggling it must reach `tauri_plugin_autostart` immediately, not on next launch. tabsy has the same pair (`get_autostart_cmd` / `set_autostart_cmd`, `tabs-manager/src-tauri/src/lib.rs:167-168`).
3. Reading the setting back has to report what the OS actually believes, not what the database wrote.

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/commands/shell.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Temp directory with a uniquifier and best-effort cleanup, as in
    /// `autofetch/local.rs` — no `tempfile` dev-dependency is added.
    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("printy-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn counts_only_files_matching_the_type_filter() {
        let dir = temp_dir("count-types");
        fs::write(dir.join("a.pdf"), b"x").unwrap();
        fs::write(dir.join("b.PDF"), b"x").unwrap();
        fs::write(dir.join("c.png"), b"x").unwrap();
        fs::write(dir.join("d.docx"), b"x").unwrap();

        let types = vec!["pdf".to_string()];
        assert_eq!(count_existing(dir.to_str().unwrap(), &types).unwrap(), 2);

        let types = vec!["pdf".to_string(), "png".to_string()];
        assert_eq!(count_existing(dir.to_str().unwrap(), &types).unwrap(), 3);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ignores_the_reserved_subdirectories() {
        let dir = temp_dir("count-reserved");
        fs::write(dir.join("a.pdf"), b"x").unwrap();
        fs::create_dir_all(dir.join("printed")).unwrap();
        fs::write(dir.join("printed").join("old.pdf"), b"x").unwrap();
        fs::create_dir_all(dir.join("failed")).unwrap();
        fs::write(dir.join("failed").join("bad.pdf"), b"x").unwrap();

        let types = vec!["pdf".to_string()];
        assert_eq!(count_existing(dir.to_str().unwrap(), &types).unwrap(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_directory_counts_as_zero_not_an_error() {
        let missing = std::env::temp_dir().join("printy-does-not-exist-9999");
        let types = vec!["pdf".to_string()];
        assert_eq!(count_existing(missing.to_str().unwrap(), &types).unwrap(), 0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test commands::shell`
Expected: FAIL — `cannot find function 'count_existing' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/commands/shell.rs`:

```rust
use crate::db::settings;
use crate::intake::scan::scan_folder;
use crate::{error::AppError, error::AppResult, AppState};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

/// Counts the files a freshly created folder would print. Uses the same
/// listing the watcher uses, so `printed/` and `failed/` are excluded and the
/// type filter matches exactly what will actually be picked up. A folder that
/// does not exist yet counts as zero rather than failing — the dialog must
/// stay usable while the user is still typing a path.
pub fn count_existing(path: &str, file_types: &[String]) -> std::io::Result<usize> {
    let root = std::path::Path::new(path);
    if !root.is_dir() {
        return Ok(0);
    }
    Ok(scan_folder(root, file_types)?.len())
}

#[tauri::command]
pub async fn count_existing_files_cmd(
    _state: State<'_, AppState>,
    path: String,
    file_types: Vec<String>,
) -> AppResult<usize> {
    Ok(count_existing(&path, &file_types)?)
}

/// Reports what the operating system believes, not what the database stored.
/// A user who removed the login item by hand must see the switch turn itself off.
#[tauri::command]
pub async fn get_autostart_cmd(app: AppHandle) -> AppResult<bool> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| AppError::Other(format!("Autostart nicht lesbar: {e}")))
}

/// Persists the setting *and* applies it, so the change takes effect now rather
/// than after the next launch.
#[tauri::command]
pub async fn set_autostart_cmd(
    state: State<'_, AppState>,
    app: AppHandle,
    enabled: bool,
) -> AppResult<()> {
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| AppError::Other(format!("Autostart nicht änderbar: {e}")))?;
    settings::set_setting(&state.db, "autostart", if enabled { "1" } else { "0" }).await?;
    Ok(())
}
```

Add `pub mod shell;` to `src-tauri/src/commands/mod.rs`, which then reads:

```rust
pub mod folders;
pub mod jobs;
pub mod printers;
pub mod settings;
pub mod shell;
```

- [ ] **Step 4: Register the commands**

In `src-tauri/src/lib.rs`, inside the `tauri::generate_handler![...]` list added by plan 1's Task 17 Step 5, add three entries after `commands::settings::set_global_paused_cmd,`:

```rust
            commands::shell::count_existing_files_cmd,
            commands::shell::get_autostart_cmd,
            commands::shell::set_autostart_cmd,
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test commands::shell`
Expected: PASS — 3 tests.

Run: `cd src-tauri && cargo build`
Expected: build succeeds.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat: add existing-file count and autostart commands for the ui shell"
```

---

### Task 4: TypeScript contract — types, API client, events and formatters

**Files:**
- Create: `src/lib/types.ts`
- Create: `src/lib/api.ts`
- Create: `src/lib/events.ts`
- Create: `src/lib/format.ts`
- Test: `src/lib/api.test.ts`
- Test: `src/lib/format.test.ts`

**Interfaces:**
- Consumes: plan 1's commands `list_printers_cmd`, `printer_capabilities_cmd`, `list_folders_cmd`, `create_folder_cmd`, `update_folder_cmd`, `delete_folder_cmd`, `set_folder_enabled_cmd`, `scan_now_cmd`, `list_jobs_cmd`, `reprint_job_cmd`, `get_settings_cmd`, `update_setting_cmd`, `set_global_paused_cmd`, `get_status_cmd`, plus Task 3's `count_existing_files_cmd`, `get_autostart_cmd`, `set_autostart_cmd`; the event topics `printy://job`, `printy://folder`, `printy://queue`.
- Produces: `lib/types::{DuplexMode, ColorMode, PostAction, FolderStatus, JobState, JobOutcome, NotificationMode, SettingKey, WatchFolder, NewFolder, PrintJob, PrinterInfo, PrinterCapabilities, AppStatus, AppSettings, JobEvent, FolderEvent, QueueEvent}`; `lib/api::api`; `lib/events::{onJobEvent, onFolderEvent, onQueueEvent}`; `lib/format::{FILE_TYPE_CHIPS, FileTypeChip, FolderStatusKind, FolderActivity, parseDbTimestamp, formatDateTime, parseFileTypes, chipIdsFromFileTypes, fileTypesFromChipIds, fileTypeLabels, folderStatusKind, folderStatusLabel, settingsSummary, folderActivity, countdownSeconds, stabilityHint, jobOutcomeLabel}`.

Every field name below is copied from plan 1's Rust structs. `serde` is used without a rename attribute, so the JSON keys are the Rust field names verbatim — snake_case on the wire, snake_case in TypeScript. Tauri converts camelCase *argument* names to snake_case parameters, which is why `printExisting` reaches `print_existing`; tabsy relies on the same conversion (`tabs-manager/src/lib/api.ts:11`).

- [ ] **Step 1: Write the failing tests**

Create `src/lib/api.test.ts`:

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { api } from "./api";
import type { NewFolder } from "./types";

// Tauri is not reachable in jsdom. tabsy never had to mock it because nothing
// under test called a command; every Printy screen does, so the whole core
// module is replaced here and in every component test.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const NEW_FOLDER: NewFolder = {
  name: "Scans",
  path: "/Users/tim/Scans",
  poll_interval_secs: 5,
  file_types: ["pdf"],
  printer_name: "Brother MFC",
  copies: 1,
  duplex: "simplex",
  color_mode: "mono",
  post_action: "move",
};

describe("api", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("calls the folder commands with the parameter names the backend declares", async () => {
    await api.listFolders();
    expect(invokeMock).toHaveBeenCalledWith("list_folders_cmd");

    await api.createFolder(NEW_FOLDER, false);
    expect(invokeMock).toHaveBeenCalledWith("create_folder_cmd", {
      folder: NEW_FOLDER,
      printExisting: false,
    });

    await api.updateFolder(7, NEW_FOLDER);
    expect(invokeMock).toHaveBeenCalledWith("update_folder_cmd", {
      id: 7,
      folder: NEW_FOLDER,
    });

    await api.setFolderEnabled(7, false);
    expect(invokeMock).toHaveBeenCalledWith("set_folder_enabled_cmd", {
      id: 7,
      enabled: false,
    });

    await api.scanNow(7);
    expect(invokeMock).toHaveBeenCalledWith("scan_now_cmd", { id: 7 });

    await api.deleteFolder(7);
    expect(invokeMock).toHaveBeenCalledWith("delete_folder_cmd", { id: 7 });
  });

  it("calls the job commands", async () => {
    await api.listJobs(true, 200);
    expect(invokeMock).toHaveBeenCalledWith("list_jobs_cmd", {
      onlyFailed: true,
      limit: 200,
    });

    await api.reprintJob(3);
    expect(invokeMock).toHaveBeenCalledWith("reprint_job_cmd", { id: 3 });
  });

  it("calls the printer commands", async () => {
    await api.listPrinters();
    expect(invokeMock).toHaveBeenCalledWith("list_printers_cmd");

    await api.printerCapabilities("Brother MFC");
    expect(invokeMock).toHaveBeenCalledWith("printer_capabilities_cmd", {
      printer: "Brother MFC",
    });
  });

  it("calls the settings, status and shell commands", async () => {
    await api.getSettings();
    expect(invokeMock).toHaveBeenCalledWith("get_settings_cmd");

    await api.updateSetting("notification_mode", "errors");
    expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
      key: "notification_mode",
      value: "errors",
    });

    await api.setGlobalPaused(true);
    expect(invokeMock).toHaveBeenCalledWith("set_global_paused_cmd", { paused: true });

    await api.getStatus();
    expect(invokeMock).toHaveBeenCalledWith("get_status_cmd");

    await api.countExistingFiles("/Users/tim/Scans", ["pdf"]);
    expect(invokeMock).toHaveBeenCalledWith("count_existing_files_cmd", {
      path: "/Users/tim/Scans",
      fileTypes: ["pdf"],
    });

    await api.getAutostart();
    expect(invokeMock).toHaveBeenCalledWith("get_autostart_cmd");

    await api.setAutostart(true);
    expect(invokeMock).toHaveBeenCalledWith("set_autostart_cmd", { enabled: true });
  });
});
```

Create `src/lib/format.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import {
  chipIdsFromFileTypes,
  countdownSeconds,
  fileTypeLabels,
  fileTypesFromChipIds,
  folderActivity,
  folderStatusKind,
  folderStatusLabel,
  formatDateTime,
  jobOutcomeLabel,
  parseDbTimestamp,
  parseFileTypes,
  settingsSummary,
  stabilityHint,
} from "./format";
import type { PrintJob, WatchFolder } from "./types";

function folder(overrides: Partial<WatchFolder> = {}): WatchFolder {
  return {
    id: 1,
    name: "Scans",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 5,
    file_types: '["pdf","png"]',
    printer_name: "Brother MFC",
    copies: 2,
    duplex: "long_edge",
    color_mode: "mono",
    post_action: "move",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
    ...overrides,
  };
}

function job(overrides: Partial<PrintJob> = {}): PrintJob {
  return {
    id: 1,
    folder_id: 1,
    file_path: "/Users/tim/Scans/a.pdf",
    file_name: "a.pdf",
    size_bytes: 1024,
    mtime_ms: 1_700_000_000_000,
    sha256: "h",
    state: "done",
    attempts: 0,
    printer_name: "Brother MFC",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    error_kind: null,
    error_message: null,
    next_attempt_at: null,
    enqueued_at: "2026-08-12 09:00:00",
    started_at: "2026-08-12 09:00:01",
    finished_at: "2026-08-12 09:00:05",
    ...overrides,
  };
}

describe("parseDbTimestamp", () => {
  it("reads sqlite datetime('now') as UTC, not as local time", () => {
    const d = parseDbTimestamp("2026-08-12 09:00:05");
    expect(d?.toISOString()).toBe("2026-08-12T09:00:05.000Z");
  });

  it("returns null for null and for junk", () => {
    expect(parseDbTimestamp(null)).toBeNull();
    expect(parseDbTimestamp("nicht ein datum")).toBeNull();
  });
});

describe("formatDateTime", () => {
  it("renders an em dash when there is no timestamp", () => {
    expect(formatDateTime(null)).toBe("—");
  });

  it("renders a German date and time otherwise", () => {
    expect(formatDateTime("2026-08-12 09:00:05")).toMatch(/12\.08\.2026/);
  });
});

describe("file types", () => {
  it("parses the JSON column and survives corruption", () => {
    expect(parseFileTypes('["pdf","png"]')).toEqual(["pdf", "png"]);
    expect(parseFileTypes("nope")).toEqual([]);
  });

  it("maps extensions onto chips and back", () => {
    expect(chipIdsFromFileTypes(["pdf", "jpg", "jpeg"])).toEqual(["pdf", "jpg"]);
    expect(fileTypesFromChipIds(["jpg", "tiff"])).toEqual(["jpg", "jpeg", "tif", "tiff"]);
  });

  it("labels the chips for display", () => {
    expect(fileTypeLabels(["pdf", "tif", "tiff"])).toEqual(["PDF", "TIFF"]);
  });
});

describe("folderStatusKind", () => {
  it("reports running for an enabled healthy folder", () => {
    expect(folderStatusKind(folder(), false)).toBe("running");
  });

  it("reports paused for a disabled folder even when the queue is held", () => {
    expect(folderStatusKind(folder({ enabled: 0 }), true)).toBe("paused");
  });

  it("reports error for a folder whose path or printer is gone", () => {
    expect(folderStatusKind(folder({ status: "path_missing" }), false)).toBe("error");
    expect(folderStatusKind(folder({ status: "printer_missing" }), false)).toBe("error");
  });

  it("reports waiting when the queue is held on a healthy enabled folder", () => {
    expect(folderStatusKind(folder(), true)).toBe("waiting");
  });

  it("labels every kind in German", () => {
    expect(folderStatusLabel("running", folder())).toBe("Aktiv");
    expect(folderStatusLabel("paused", folder({ enabled: 0 }))).toBe("Pausiert");
    expect(folderStatusLabel("waiting", folder())).toBe("Wartet auf Drucker");
    expect(folderStatusLabel("error", folder({ status: "path_missing" }))).toBe(
      "Ordner nicht gefunden",
    );
    expect(folderStatusLabel("error", folder({ status: "printer_missing" }))).toBe(
      "Drucker nicht mehr installiert",
    );
  });
});

describe("settingsSummary", () => {
  it("summarises printer and print options in German", () => {
    expect(settingsSummary(folder())).toBe(
      "Brother MFC · 2 Kopien · Duplex (lange Kante) · Schwarz-weiß · verschieben",
    );
  });

  it("uses the singular for a single copy and names simplex plainly", () => {
    expect(settingsSummary(folder({ copies: 1, duplex: "simplex", color_mode: "color" }))).toBe(
      "Brother MFC · 1 Kopie · Einseitig · Farbe · verschieben",
    );
  });
});

describe("folderActivity", () => {
  const now = Date.parse("2026-08-12T12:00:00Z");

  it("counts today's completed jobs for this folder only", () => {
    const jobs = [
      job({ id: 1, folder_id: 1, finished_at: "2026-08-12 09:00:05" }),
      job({ id: 2, folder_id: 1, finished_at: "2026-08-12 10:00:05" }),
      job({ id: 3, folder_id: 2, finished_at: "2026-08-12 10:30:05" }),
      job({ id: 4, folder_id: 1, finished_at: "2026-08-11 10:00:05" }),
    ];
    expect(folderActivity(jobs, 1, now).printedToday).toBe(2);
  });

  it("reports the most recent finish as the last activity", () => {
    const jobs = [
      job({ id: 1, folder_id: 1, finished_at: "2026-08-12 09:00:05" }),
      job({ id: 2, folder_id: 1, finished_at: "2026-08-12 10:00:05" }),
    ];
    expect(folderActivity(jobs, 1, now).lastActivity).toBe("2026-08-12 10:00:05");
  });

  it("surfaces the earliest pending retry so the card can count down", () => {
    const jobs = [
      job({ id: 1, folder_id: 1, state: "retrying", next_attempt_at: now + 30_000 }),
      job({ id: 2, folder_id: 1, state: "retrying", next_attempt_at: now + 10_000 }),
    ];
    expect(folderActivity(jobs, 1, now).nextAttemptAt).toBe(now + 10_000);
  });

  it("is empty when the folder has no jobs at all", () => {
    expect(folderActivity([], 1, now)).toEqual({
      printedToday: 0,
      lastActivity: null,
      nextAttemptAt: null,
      failedCount: 0,
    });
  });
});

describe("countdownSeconds", () => {
  it("rounds up to whole seconds and never goes negative", () => {
    expect(countdownSeconds(1000, 0)).toBe(1);
    expect(countdownSeconds(1500, 0)).toBe(2);
    expect(countdownSeconds(500, 1000)).toBe(0);
    expect(countdownSeconds(null, 0)).toBeNull();
  });
});

describe("stabilityHint", () => {
  it("names the two-tick cost in seconds", () => {
    expect(stabilityHint(5)).toContain("10 Sekunden");
  });

  it("switches to minutes once the delay passes a minute", () => {
    expect(stabilityHint(60)).toContain("2 Minuten");
  });
});

describe("jobOutcomeLabel", () => {
  it("labels every job state in German", () => {
    expect(jobOutcomeLabel("done")).toBe("Gedruckt");
    expect(jobOutcomeLabel("failed")).toBe("Fehlgeschlagen");
    expect(jobOutcomeLabel("printing")).toBe("Druckt");
    expect(jobOutcomeLabel("queued")).toBe("Wartet");
    expect(jobOutcomeLabel("retrying")).toBe("Wiederholt");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/api.test.ts src/lib/format.test.ts`
Expected: FAIL — `Failed to resolve import "./api"` and `Failed to resolve import "./format"`.

- [ ] **Step 3: Write the types**

Create `src/lib/types.ts`. Field names mirror plan 1's `db::models::WatchFolder`, `db::models::PrintJob`, `db::folders::NewFolder`, `print::PrinterInfo`, `print::PrinterCapabilities` and `commands::jobs::AppStatus`.

```ts
export type DuplexMode = "simplex" | "long_edge" | "short_edge";
export type ColorMode = "color" | "mono";
export type PostAction = "move" | "keep" | "delete";
export type FolderStatus = "ok" | "path_missing" | "printer_missing";
export type JobState = "queued" | "printing" | "retrying" | "done" | "failed";
export type NotificationMode = "all" | "errors" | "off";

/** The keys `get_settings_cmd` always returns, defaulted server-side. */
export type SettingKey =
  | "notification_mode"
  | "autostart"
  | "start_minimized"
  | "theme"
  | "sumatra_path"
  | "user_paused";

/** A row of `watch_folder`. `enabled` is SQLite's integer boolean. */
export interface WatchFolder {
  id: number;
  name: string;
  path: string;
  enabled: number;
  poll_interval_secs: number;
  /** JSON array of lowercase extensions, e.g. `["pdf","png"]`. */
  file_types: string;
  printer_name: string;
  copies: number;
  duplex: DuplexMode;
  color_mode: ColorMode;
  post_action: PostAction;
  status: FolderStatus;
  created_at: string;
  updated_at: string;
}

/** The payload `create_folder_cmd` and `update_folder_cmd` deserialize. */
export interface NewFolder {
  name: string;
  path: string;
  poll_interval_secs: number;
  file_types: string[];
  printer_name: string;
  copies: number;
  duplex: DuplexMode;
  color_mode: ColorMode;
  post_action: PostAction;
}

/** A row of `print_job` — queue, history and dedup ledger in one. */
export interface PrintJob {
  id: number;
  folder_id: number;
  file_path: string;
  file_name: string;
  size_bytes: number;
  mtime_ms: number;
  sha256: string;
  state: JobState;
  attempts: number;
  printer_name: string;
  copies: number;
  duplex: DuplexMode;
  color_mode: ColorMode;
  error_kind: string | null;
  error_message: string | null;
  next_attempt_at: number | null;
  enqueued_at: string;
  started_at: string | null;
  finished_at: string | null;
}

export interface PrinterInfo {
  name: string;
  is_default: boolean;
}

export interface PrinterCapabilities {
  duplex: boolean;
  color: boolean;
  copies: boolean;
}

export interface AppStatus {
  active_folders: number;
  printed_today: number;
  waiting: number;
  failed: number;
  user_paused: boolean;
}

export type AppSettings = Record<SettingKey, string>;

/** Payload of `printy://job`. The debug name of `queue::worker::QueueOutcome`. */
export type JobOutcome = "Printed" | "Retried" | "Failed";
export interface JobEvent {
  outcome: JobOutcome;
}

/** Payload of `printy://folder`. */
export interface FolderEvent {
  folder_id: number;
  enqueued: number;
  path_missing: boolean;
}

/** Payload of `printy://queue`. `reason` is only present while held. */
export interface QueueEvent {
  held: boolean;
  reason?: string;
}
```

- [ ] **Step 4: Write the API client and the event helpers**

Create `src/lib/api.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  AppStatus,
  NewFolder,
  PrintJob,
  PrinterCapabilities,
  PrinterInfo,
  SettingKey,
  WatchFolder,
} from "./types";

/**
 * The only place in the frontend that talks to Tauri. Command names and
 * parameter names come from the core engine plan's Task 17; camelCase argument
 * keys are converted to snake_case parameters by Tauri itself.
 */
export const api = {
  listPrinters: () => invoke<PrinterInfo[]>("list_printers_cmd"),
  printerCapabilities: (printer: string) =>
    invoke<PrinterCapabilities>("printer_capabilities_cmd", { printer }),

  listFolders: () => invoke<WatchFolder[]>("list_folders_cmd"),
  createFolder: (folder: NewFolder, printExisting: boolean) =>
    invoke<WatchFolder>("create_folder_cmd", { folder, printExisting }),
  updateFolder: (id: number, folder: NewFolder) =>
    invoke<WatchFolder>("update_folder_cmd", { id, folder }),
  deleteFolder: (id: number) => invoke<void>("delete_folder_cmd", { id }),
  setFolderEnabled: (id: number, enabled: boolean) =>
    invoke<void>("set_folder_enabled_cmd", { id, enabled }),
  scanNow: (id: number) => invoke<number>("scan_now_cmd", { id }),

  listJobs: (onlyFailed: boolean, limit: number) =>
    invoke<PrintJob[]>("list_jobs_cmd", { onlyFailed, limit }),
  reprintJob: (id: number) => invoke<void>("reprint_job_cmd", { id }),

  getSettings: () => invoke<AppSettings>("get_settings_cmd"),
  updateSetting: (key: SettingKey, value: string) =>
    invoke<void>("update_setting_cmd", { key, value }),
  setGlobalPaused: (paused: boolean) => invoke<void>("set_global_paused_cmd", { paused }),
  getStatus: () => invoke<AppStatus>("get_status_cmd"),

  countExistingFiles: (path: string, fileTypes: string[]) =>
    invoke<number>("count_existing_files_cmd", { path, fileTypes }),
  getAutostart: () => invoke<boolean>("get_autostart_cmd"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart_cmd", { enabled }),
};
```

Create `src/lib/events.ts`:

```ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { FolderEvent, JobEvent, QueueEvent } from "./types";

export function onJobEvent(cb: (e: JobEvent) => void): Promise<UnlistenFn> {
  return listen<JobEvent>("printy://job", (e) => cb(e.payload));
}

export function onFolderEvent(cb: (e: FolderEvent) => void): Promise<UnlistenFn> {
  return listen<FolderEvent>("printy://folder", (e) => cb(e.payload));
}

export function onQueueEvent(cb: (e: QueueEvent) => void): Promise<UnlistenFn> {
  return listen<QueueEvent>("printy://queue", (e) => cb(e.payload));
}
```

- [ ] **Step 5: Write the formatters**

Create `src/lib/format.ts`:

```ts
import type { JobState, PrintJob, WatchFolder } from "./types";

export interface FileTypeChip {
  id: string;
  label: string;
  /** Every extension this chip stands for, lowercase and without a dot. */
  extensions: string[];
}

/** The four formats the print pipeline supports (spec section 2). */
export const FILE_TYPE_CHIPS: ReadonlyArray<FileTypeChip> = [
  { id: "pdf", label: "PDF", extensions: ["pdf"] },
  { id: "jpg", label: "JPG", extensions: ["jpg", "jpeg"] },
  { id: "png", label: "PNG", extensions: ["png"] },
  { id: "tiff", label: "TIFF", extensions: ["tif", "tiff"] },
];

/**
 * SQLite's `datetime('now')` writes `YYYY-MM-DD HH:MM:SS` in UTC. Handing that
 * straight to `new Date` makes V8 read it as *local* time, which shifts every
 * timestamp by the timezone offset. Normalise explicitly.
 */
export function parseDbTimestamp(value: string | null): Date | null {
  if (value === null || value.trim() === "") return null;
  const normalized = value.includes("T") ? value : `${value.replace(" ", "T")}Z`;
  const d = new Date(normalized);
  return Number.isNaN(d.getTime()) ? null : d;
}

export function formatDateTime(value: string | null): string {
  const d = parseDbTimestamp(value);
  if (d === null) return "—";
  return d.toLocaleString("de-DE", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** The `file_types` column is JSON; a corrupted value must not blank the screen. */
export function parseFileTypes(json: string): string[] {
  try {
    const parsed: unknown = JSON.parse(json);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((v): v is string => typeof v === "string");
  } catch {
    return [];
  }
}

export function chipIdsFromFileTypes(fileTypes: string[]): string[] {
  const set = new Set(fileTypes.map((t) => t.toLowerCase()));
  return FILE_TYPE_CHIPS.filter((c) => c.extensions.some((e) => set.has(e))).map((c) => c.id);
}

export function fileTypesFromChipIds(chipIds: string[]): string[] {
  return FILE_TYPE_CHIPS.filter((c) => chipIds.includes(c.id)).flatMap((c) => c.extensions);
}

export function fileTypeLabels(fileTypes: string[]): string[] {
  return chipIdsFromFileTypes(fileTypes).map(
    (id) => FILE_TYPE_CHIPS.find((c) => c.id === id)?.label ?? id.toUpperCase(),
  );
}

export type FolderStatusKind = "running" | "error" | "paused" | "waiting";

/**
 * The user's per-folder pause wins over everything: a folder the user switched
 * off must not blink amber because some other printer is unreachable. A config
 * error outranks the printer hold, because it needs the user rather than time.
 */
export function folderStatusKind(folder: WatchFolder, queueHeld: boolean): FolderStatusKind {
  if (folder.enabled === 0) return "paused";
  if (folder.status !== "ok") return "error";
  if (queueHeld) return "waiting";
  return "running";
}

export function folderStatusLabel(kind: FolderStatusKind, folder: WatchFolder): string {
  switch (kind) {
    case "running":
      return "Aktiv";
    case "paused":
      return "Pausiert";
    case "waiting":
      return "Wartet auf Drucker";
    case "error":
      return folder.status === "path_missing"
        ? "Ordner nicht gefunden"
        : "Drucker nicht mehr installiert";
  }
}

const DUPLEX_LABEL: Record<WatchFolder["duplex"], string> = {
  simplex: "Einseitig",
  long_edge: "Duplex (lange Kante)",
  short_edge: "Duplex (kurze Kante)",
};

const COLOR_LABEL: Record<WatchFolder["color_mode"], string> = {
  color: "Farbe",
  mono: "Schwarz-weiß",
};

const POST_ACTION_LABEL: Record<WatchFolder["post_action"], string> = {
  move: "verschieben",
  keep: "liegen lassen",
  delete: "löschen",
};

export function settingsSummary(folder: WatchFolder): string {
  const copies = folder.copies === 1 ? "1 Kopie" : `${folder.copies} Kopien`;
  return [
    folder.printer_name,
    copies,
    DUPLEX_LABEL[folder.duplex],
    COLOR_LABEL[folder.color_mode],
    POST_ACTION_LABEL[folder.post_action],
  ].join(" · ");
}

export interface FolderActivity {
  printedToday: number;
  /** Raw database timestamp of the newest finished job, or null. */
  lastActivity: string | null;
  /** Epoch ms of the soonest pending retry in this folder, or null. */
  nextAttemptAt: number | null;
  failedCount: number;
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

export function folderActivity(
  jobs: PrintJob[],
  folderId: number,
  nowMs: number,
): FolderActivity {
  const today = new Date(nowMs);
  const mine = jobs.filter((j) => j.folder_id === folderId);

  let printedToday = 0;
  let failedCount = 0;
  let lastActivity: string | null = null;
  let lastMs = -1;
  let nextAttemptAt: number | null = null;

  for (const j of mine) {
    if (j.state === "failed") failedCount += 1;
    if (j.state === "retrying" && j.next_attempt_at !== null) {
      if (nextAttemptAt === null || j.next_attempt_at < nextAttemptAt) {
        nextAttemptAt = j.next_attempt_at;
      }
    }
    const finished = parseDbTimestamp(j.finished_at);
    if (finished === null) continue;
    if (j.state === "done" && isSameDay(finished, today)) printedToday += 1;
    if (finished.getTime() > lastMs) {
      lastMs = finished.getTime();
      lastActivity = j.finished_at;
    }
  }

  return { printedToday, lastActivity, nextAttemptAt, failedCount };
}

export function countdownSeconds(nextAttemptAt: number | null, nowMs: number): number | null {
  if (nextAttemptAt === null) return null;
  return Math.max(0, Math.ceil((nextAttemptAt - nowMs) / 1000));
}

/**
 * The stability check needs two consecutive unchanged scans, so a file lands on
 * paper up to two intervals after it appears. The interval field has to say so —
 * a 30-second interval quietly costing a minute is the kind of surprise that
 * makes people distrust the app.
 */
export function stabilityHint(intervalSecs: number): string {
  const worst = Math.max(1, intervalSecs) * 2;
  const amount =
    worst >= 120
      ? `${Math.round(worst / 60)} Minuten`
      : worst === 60
        ? "1 Minute"
        : `${worst} Sekunden`;
  return (
    `Eine Datei gilt erst als fertig, wenn sie sich zwei Scans lang nicht mehr ändert. ` +
    `Neue Dateien werden deshalb bis zu ${amount} nach dem Auftauchen gedruckt.`
  );
}

const JOB_STATE_LABEL: Record<JobState, string> = {
  queued: "Wartet",
  printing: "Druckt",
  retrying: "Wiederholt",
  done: "Gedruckt",
  failed: "Fehlgeschlagen",
};

export function jobOutcomeLabel(state: JobState): string {
  return JOB_STATE_LABEL[state];
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npx vitest run src/lib/api.test.ts src/lib/format.test.ts`
Expected: PASS — 4 api tests and 22 format tests.

Run: `npx tsc --noEmit`
Expected: no type errors.

- [ ] **Step 7: Commit**

```bash
git add src/lib
git commit -m "feat: add typed tauri api client, event helpers and formatters"
```

---

### Task 5: FolderCard

**Files:**
- Create: `src/components/FolderCard.tsx`
- Test: `src/components/FolderCard.test.tsx`

**Interfaces:**
- Consumes: `lib/types::{WatchFolder}`; `lib/format::{FolderActivity, folderActivity, folderStatusKind, folderStatusLabel, settingsSummary, fileTypeLabels, parseFileTypes, formatDateTime, countdownSeconds}`; `routes/screens.css` classes `.folder-card`, `.status-dot`, `.folder-actions`.
- Produces: `components/FolderCard` — `default function FolderCard(props: FolderCardProps): JSX.Element`; `FolderCardProps`.

`FolderCard` is purely presentational: it receives data and callbacks and never calls the backend. That keeps its test free of a query client and of a Tauri mock, and it keeps every mutation in one place — the Ordner screen of Task 7.

- [ ] **Step 1: Write the failing test**

Create `src/components/FolderCard.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import "@testing-library/jest-dom";
import type { ComponentProps } from "react";
import FolderCard from "./FolderCard";
import type { FolderActivity } from "../lib/format";
import type { WatchFolder } from "../lib/types";

const NOW = Date.parse("2026-08-12T12:00:00Z");

function folder(overrides: Partial<WatchFolder> = {}): WatchFolder {
  return {
    id: 1,
    name: "Scanner",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 5,
    file_types: '["pdf","png"]',
    printer_name: "Brother MFC",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    post_action: "move",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
    ...overrides,
  };
}

function activity(overrides: Partial<FolderActivity> = {}): FolderActivity {
  return {
    printedToday: 0,
    lastActivity: null,
    nextAttemptAt: null,
    failedCount: 0,
    ...overrides,
  };
}

function renderCard(props: Partial<ComponentProps<typeof FolderCard>> = {}) {
  const handlers = {
    onToggleEnabled: vi.fn(),
    onScanNow: vi.fn(),
    onEdit: vi.fn(),
    onReveal: vi.fn(),
  };
  render(
    <FolderCard
      folder={folder()}
      activity={activity()}
      queueHeld={false}
      nowMs={NOW}
      {...handlers}
      {...props}
    />,
  );
  return handlers;
}

describe("FolderCard status", () => {
  it("shows a mint running dot for an enabled healthy folder", () => {
    renderCard();
    expect(screen.getByTestId("status-dot")).toHaveClass("running");
    expect(screen.getByText("Aktiv")).toBeInTheDocument();
  });

  it("shows a grey paused dot for a disabled folder", () => {
    renderCard({ folder: folder({ enabled: 0 }) });
    expect(screen.getByTestId("status-dot")).toHaveClass("paused");
    expect(screen.getByText("Pausiert")).toBeInTheDocument();
  });

  it("shows a coral error dot and names the reason for a missing path", () => {
    renderCard({ folder: folder({ status: "path_missing" }) });
    expect(screen.getByTestId("status-dot")).toHaveClass("error");
    expect(screen.getByText("Ordner nicht gefunden")).toBeInTheDocument();
  });

  it("shows a coral error dot and names the reason for a missing printer", () => {
    renderCard({ folder: folder({ status: "printer_missing" }) });
    expect(screen.getByTestId("status-dot")).toHaveClass("error");
    expect(screen.getByText("Drucker nicht mehr installiert")).toBeInTheDocument();
  });

  it("distinguishes a printer hold from a user pause", () => {
    renderCard({ queueHeld: true });
    expect(screen.getByTestId("status-dot")).toHaveClass("waiting");
    expect(screen.getByText("Wartet auf Drucker")).toBeInTheDocument();
  });
});

describe("FolderCard content", () => {
  it("renders name, type badges, monospace path and settings summary", () => {
    renderCard();
    expect(screen.getByText("Scanner")).toBeInTheDocument();
    const badges = screen.getByTestId("type-badges");
    expect(within(badges).getByText("PDF")).toBeInTheDocument();
    expect(within(badges).getByText("PNG")).toBeInTheDocument();
    expect(screen.getByTestId("folder-path")).toHaveTextContent("/Users/tim/Scans");
    expect(screen.getByTestId("folder-path")).toHaveClass("mono");
    expect(
      screen.getByText("Brother MFC · 1 Kopie · Einseitig · Schwarz-weiß · verschieben"),
    ).toBeInTheDocument();
  });

  it("reports today's count and the last activity", () => {
    renderCard({
      activity: activity({ printedToday: 12, lastActivity: "2026-08-12 09:00:05" }),
    });
    expect(screen.getByText("12 heute gedruckt")).toBeInTheDocument();
    expect(screen.getByTestId("last-activity")).toHaveTextContent("12.08.2026");
  });

  it("counts down to the next retry when a job is held", () => {
    renderCard({ activity: activity({ nextAttemptAt: NOW + 30_000 }) });
    expect(screen.getByTestId("retry-countdown")).toHaveTextContent(
      "Nächster Versuch in 30 s",
    );
  });

  it("hides the countdown when nothing is pending", () => {
    renderCard();
    expect(screen.queryByTestId("retry-countdown")).not.toBeInTheDocument();
  });
});

describe("FolderCard actions", () => {
  it("offers pause, scan now, edit and reveal, and reports each one", () => {
    const h = renderCard();

    fireEvent.click(screen.getByRole("button", { name: "Pausieren" }));
    expect(h.onToggleEnabled).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Jetzt scannen" }));
    expect(h.onScanNow).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Bearbeiten" }));
    expect(h.onEdit).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Im Explorer öffnen" }));
    expect(h.onReveal).toHaveBeenCalledTimes(1);
  });

  it("offers resuming instead of pausing on a disabled folder", () => {
    renderCard({ folder: folder({ enabled: 0 }) });
    expect(screen.getByRole("button", { name: "Fortsetzen" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Pausieren" })).not.toBeInTheDocument();
  });

  it("locks every action while a mutation is in flight", () => {
    renderCard({ busy: true });
    expect(screen.getByRole("button", { name: "Pausieren" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Jetzt scannen" })).toBeDisabled();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/components/FolderCard.test.tsx`
Expected: FAIL — `Failed to resolve import "./FolderCard"`.

- [ ] **Step 3: Write the component**

Create `src/components/FolderCard.tsx`:

```tsx
import type { JSX } from "react";
import type { WatchFolder } from "../lib/types";
import {
  countdownSeconds,
  fileTypeLabels,
  folderStatusKind,
  folderStatusLabel,
  formatDateTime,
  parseFileTypes,
  settingsSummary,
  type FolderActivity,
} from "../lib/format";

export interface FolderCardProps {
  folder: WatchFolder;
  activity: FolderActivity;
  /** True while the whole queue waits for a printer to come back. */
  queueHeld: boolean;
  /** Injected rather than read from the clock, so the countdown is testable. */
  nowMs: number;
  busy?: boolean;
  onToggleEnabled: (folder: WatchFolder) => void;
  onScanNow: (folder: WatchFolder) => void;
  onEdit: (folder: WatchFolder) => void;
  onReveal: (folder: WatchFolder) => void;
}

/**
 * One watched folder at a glance. Presentational only — every action is handed
 * back to the Ordner screen, which owns the mutations.
 */
export default function FolderCard({
  folder,
  activity,
  queueHeld,
  nowMs,
  busy = false,
  onToggleEnabled,
  onScanNow,
  onEdit,
  onReveal,
}: FolderCardProps): JSX.Element {
  const kind = folderStatusKind(folder, queueHeld);
  const enabled = folder.enabled !== 0;
  const labels = fileTypeLabels(parseFileTypes(folder.file_types));
  const countdown = countdownSeconds(activity.nextAttemptAt, nowMs);

  return (
    <li className={`folder-card${kind === "error" ? " has-error" : ""}`}>
      <div className="folder-head">
        <span
          className={`status-dot ${kind}`}
          data-testid="status-dot"
          aria-hidden="true"
        />
        <span className="folder-name">{folder.name}</span>
        <span className="folder-state">{folderStatusLabel(kind, folder)}</span>
        <span className="chip-row" data-testid="type-badges">
          {labels.map((label) => (
            <span key={label} className="badge">
              {label}
            </span>
          ))}
        </span>
      </div>

      <span className="mono muted" data-testid="folder-path">
        {folder.path}
      </span>

      <span className="folder-summary">{settingsSummary(folder)}</span>

      {kind === "error" && (
        <span className="folder-error">
          {folder.status === "path_missing"
            ? "Der Ordner existiert nicht mehr. Die Überwachung ist gestoppt."
            : "Der eingestellte Drucker ist nicht mehr installiert."}
        </span>
      )}

      {countdown !== null && (
        <span className="folder-error" data-testid="retry-countdown">
          Nächster Versuch in {countdown} s
        </span>
      )}

      <div className="folder-meta">
        <span>{activity.printedToday} heute gedruckt</span>
        <span data-testid="last-activity">
          Zuletzt: {formatDateTime(activity.lastActivity)}
        </span>
        {activity.failedCount > 0 && (
          <span>{activity.failedCount} fehlgeschlagen</span>
        )}
      </div>

      <div className="folder-actions">
        <button
          type="button"
          className="link-btn"
          disabled={busy}
          onClick={() => onToggleEnabled(folder)}
        >
          {enabled ? "Pausieren" : "Fortsetzen"}
        </button>
        <button
          type="button"
          className="link-btn"
          disabled={busy}
          onClick={() => onScanNow(folder)}
        >
          Jetzt scannen
        </button>
        <button type="button" className="link-btn" onClick={() => onEdit(folder)}>
          Bearbeiten
        </button>
        <button type="button" className="link-btn" onClick={() => onReveal(folder)}>
          Im Explorer öffnen
        </button>
      </div>
    </li>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/components/FolderCard.test.tsx`
Expected: PASS — 12 tests.

- [ ] **Step 5: Commit**

```bash
git add src/components/FolderCard.tsx src/components/FolderCard.test.tsx
git commit -m "feat: add folder card with status, activity and per-folder actions"
```

---

### Task 6: FolderDialog

**Files:**
- Create: `src/components/FolderDialog.tsx`
- Test: `src/components/FolderDialog.test.tsx`

**Interfaces:**
- Consumes: `lib/types::{NewFolder, WatchFolder, PrinterInfo, PrinterCapabilities, DuplexMode, ColorMode, PostAction}`; `lib/format::{FILE_TYPE_CHIPS, chipIdsFromFileTypes, fileTypesFromChipIds, parseFileTypes, stabilityHint}`; `@tauri-apps/plugin-dialog`'s `open`.
- Produces: `components/FolderDialog` — `default function FolderDialog(props: FolderDialogProps): JSX.Element`; `FolderDialogProps`; `FolderDialogResult { folder: NewFolder; printExisting: boolean }`.

Two rules drive this component. Controls the selected printer cannot honour are **disabled and explained, never hidden and never silently ignored** — the disabling is driven by `printer_capabilities_cmd`. And "vorhandene N Dateien jetzt mitdrucken" defaults to **off**: adding a folder marks what is already in it as seen, and printing it has to be an explicit choice.

- [ ] **Step 1: Write the failing test**

Create `src/components/FolderDialog.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@testing-library/jest-dom";
import type { ComponentProps } from "react";
import FolderDialog from "./FolderDialog";
import type { PrinterCapabilities, PrinterInfo, WatchFolder } from "../lib/types";

const openMock = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: (...a: unknown[]) => openMock(...a) }));

const PRINTERS: PrinterInfo[] = [
  { name: "Brother MFC", is_default: false },
  { name: "HP LaserJet", is_default: true },
];

const ALL_CAPS: PrinterCapabilities = { duplex: true, color: true, copies: true };
const NO_CAPS: PrinterCapabilities = { duplex: false, color: false, copies: false };

function existing(folder: Partial<WatchFolder> = {}): WatchFolder {
  return {
    id: 4,
    name: "Scanner",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 30,
    file_types: '["pdf"]',
    printer_name: "Brother MFC",
    copies: 3,
    duplex: "long_edge",
    color_mode: "color",
    post_action: "keep",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
    ...folder,
  };
}

function renderDialog(props: Partial<ComponentProps<typeof FolderDialog>> = {}) {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  const onPrinterChange = vi.fn();
  render(
    <FolderDialog
      folder={null}
      printers={PRINTERS}
      capabilities={ALL_CAPS}
      countExisting={async () => 23}
      onPrinterChange={onPrinterChange}
      onCancel={onCancel}
      onSubmit={onSubmit}
      {...props}
    />,
  );
  return { onSubmit, onCancel, onPrinterChange };
}

describe("FolderDialog printer capabilities", () => {
  it("disables unsupported controls instead of hiding them", () => {
    renderDialog({ capabilities: NO_CAPS });

    const duplex = screen.getByLabelText("Duplex");
    const color = screen.getByLabelText("Farbe");
    const copies = screen.getByLabelText("Kopien");

    expect(duplex).toBeInTheDocument();
    expect(duplex).toBeDisabled();
    expect(color).toBeDisabled();
    expect(copies).toBeDisabled();

    expect(
      screen.getAllByText("Dieser Drucker unterstützt das nicht.").length,
    ).toBe(3);
  });

  it("leaves supported controls enabled", () => {
    renderDialog({ capabilities: ALL_CAPS });
    expect(screen.getByLabelText("Duplex")).toBeEnabled();
    expect(screen.getByLabelText("Farbe")).toBeEnabled();
    expect(screen.getByLabelText("Kopien")).toBeEnabled();
    expect(screen.queryByText("Dieser Drucker unterstützt das nicht.")).not.toBeInTheDocument();
  });

  it("preselects the system default printer when creating", () => {
    renderDialog();
    expect(screen.getByLabelText("Drucker")).toHaveValue("HP LaserJet");
  });

  it("keeps the folder's own printer when editing", () => {
    renderDialog({ folder: existing() });
    expect(screen.getByLabelText("Drucker")).toHaveValue("Brother MFC");
  });

  it("asks the parent to refetch capabilities when the printer changes", () => {
    const { onPrinterChange } = renderDialog();
    fireEvent.change(screen.getByLabelText("Drucker"), {
      target: { value: "Brother MFC" },
    });
    expect(onPrinterChange).toHaveBeenCalledWith("Brother MFC");
  });
});

describe("FolderDialog print-existing checkbox", () => {
  it("defaults to off and names the number of files found", async () => {
    renderDialog();
    const box = screen.getByLabelText(/jetzt mitdrucken/);
    expect(box).not.toBeChecked();
    await waitFor(() =>
      expect(screen.getByLabelText("Vorhandene 23 Dateien jetzt mitdrucken")).toBeInTheDocument(),
    );
  });

  it("submits print_existing false unless the user ticks it", async () => {
    const { onSubmit } = renderDialog();
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Scanner" } });
    fireEvent.change(screen.getByLabelText("Ordner"), {
      target: { value: "/Users/tim/Scans" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Anlegen" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].printExisting).toBe(false);
    expect(onSubmit.mock.calls[0][0].folder).toEqual({
      name: "Scanner",
      path: "/Users/tim/Scans",
      poll_interval_secs: 5,
      file_types: ["pdf"],
      printer_name: "HP LaserJet",
      copies: 1,
      duplex: "simplex",
      color_mode: "mono",
      post_action: "move",
    });
  });

  it("submits print_existing true once the user opts in", async () => {
    const { onSubmit } = renderDialog();
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Scanner" } });
    fireEvent.change(screen.getByLabelText("Ordner"), {
      target: { value: "/Users/tim/Scans" },
    });
    fireEvent.click(screen.getByLabelText(/jetzt mitdrucken/));
    fireEvent.click(screen.getByRole("button", { name: "Anlegen" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].printExisting).toBe(true);
  });

  it("does not offer the checkbox when editing an existing folder", () => {
    renderDialog({ folder: existing() });
    expect(screen.queryByLabelText(/jetzt mitdrucken/)).not.toBeInTheDocument();
  });
});

describe("FolderDialog form", () => {
  it("spells out the two-interval stability delay", () => {
    renderDialog();
    expect(screen.getByTestId("interval-hint")).toHaveTextContent(
      "bis zu 10 Sekunden nach dem Auftauchen gedruckt",
    );
  });

  it("uses the native folder picker", async () => {
    openMock.mockResolvedValue("/Users/tim/Belege");
    renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "Durchsuchen …" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Ordner")).toHaveValue("/Users/tim/Belege"),
    );
    expect(openMock).toHaveBeenCalledWith({ directory: true, multiple: false });
  });

  it("refuses to submit without a name, a path and at least one file type", () => {
    const { onSubmit } = renderDialog();
    expect(screen.getByRole("button", { name: "Anlegen" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Scanner" } });
    expect(screen.getByRole("button", { name: "Anlegen" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Ordner"), { target: { value: "/tmp/x" } });
    expect(screen.getByRole("button", { name: "Anlegen" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "PDF" }));
    expect(screen.getByRole("button", { name: "Anlegen" })).toBeDisabled();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("prefills every field when editing", () => {
    renderDialog({ folder: existing() });
    expect(screen.getByLabelText("Name")).toHaveValue("Scanner");
    expect(screen.getByLabelText("Ordner")).toHaveValue("/Users/tim/Scans");
    expect(screen.getByLabelText("Prüfintervall (Sekunden)")).toHaveValue(30);
    expect(screen.getByLabelText("Kopien")).toHaveValue(3);
    expect(screen.getByLabelText("Duplex")).toHaveValue("long_edge");
    expect(screen.getByLabelText("Farbe")).toHaveValue("color");
    expect(screen.getByLabelText("Nach dem Druck")).toHaveValue("keep");
    expect(screen.getByRole("button", { name: "Speichern" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/components/FolderDialog.test.tsx`
Expected: FAIL — `Failed to resolve import "./FolderDialog"`.

- [ ] **Step 3: Write the component**

Create `src/components/FolderDialog.tsx`:

```tsx
import { useEffect, useState, type FormEvent, type JSX } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ColorMode,
  DuplexMode,
  NewFolder,
  PostAction,
  PrinterCapabilities,
  PrinterInfo,
  WatchFolder,
} from "../lib/types";
import {
  FILE_TYPE_CHIPS,
  chipIdsFromFileTypes,
  fileTypesFromChipIds,
  parseFileTypes,
  stabilityHint,
} from "../lib/format";

export interface FolderDialogResult {
  folder: NewFolder;
  printExisting: boolean;
}

export interface FolderDialogProps {
  /** null creates a folder; a row edits it. */
  folder: WatchFolder | null;
  printers: PrinterInfo[];
  /** null while the capability probe is still running. */
  capabilities: PrinterCapabilities | null;
  /** Injected so the dialog stays free of query wiring and easy to test. */
  countExisting: (path: string, fileTypes: string[]) => Promise<number>;
  onPrinterChange: (printer: string) => void;
  onCancel: () => void;
  onSubmit: (result: FolderDialogResult) => void;
  saving?: boolean;
}

const UNSUPPORTED = "Dieser Drucker unterstützt das nicht.";

interface FormState {
  name: string;
  path: string;
  pollInterval: string;
  chipIds: string[];
  printerName: string;
  copies: string;
  duplex: DuplexMode;
  colorMode: ColorMode;
  postAction: PostAction;
}

function initialState(folder: WatchFolder | null, printers: PrinterInfo[]): FormState {
  if (folder !== null) {
    return {
      name: folder.name,
      path: folder.path,
      pollInterval: String(folder.poll_interval_secs),
      chipIds: chipIdsFromFileTypes(parseFileTypes(folder.file_types)),
      printerName: folder.printer_name,
      copies: String(folder.copies),
      duplex: folder.duplex,
      colorMode: folder.color_mode,
      postAction: folder.post_action,
    };
  }
  const preselected =
    printers.find((p) => p.is_default)?.name ?? printers[0]?.name ?? "";
  return {
    name: "",
    path: "",
    pollInterval: "5",
    chipIds: ["pdf"],
    printerName: preselected,
    copies: "1",
    duplex: "simplex",
    colorMode: "mono",
    postAction: "move",
  };
}

export default function FolderDialog({
  folder,
  printers,
  capabilities,
  countExisting,
  onPrinterChange,
  onCancel,
  onSubmit,
  saving = false,
}: FolderDialogProps): JSX.Element {
  const creating = folder === null;
  const [form, setForm] = useState<FormState>(() => initialState(folder, printers));
  const [printExisting, setPrintExisting] = useState(false);
  const [existingCount, setExistingCount] = useState<number | null>(null);

  const fileTypes = fileTypesFromChipIds(form.chipIds);
  const intervalSecs = Math.max(1, Number.parseInt(form.pollInterval, 10) || 1);

  // Only meaningful while creating: an existing folder has long since been
  // catalogued, and re-offering to print its contents would be a paper trap.
  useEffect(() => {
    if (!creating || form.path.trim() === "" || fileTypes.length === 0) {
      setExistingCount(null);
      return;
    }
    let cancelled = false;
    void countExisting(form.path.trim(), fileTypes)
      .then((n) => {
        if (!cancelled) setExistingCount(n);
      })
      .catch(() => {
        if (!cancelled) setExistingCount(null);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [creating, form.path, form.chipIds.join(",")]);

  // Capabilities are unknown while the probe runs; assume nothing is blocked
  // rather than greying out a control the printer may well support.
  const canDuplex = capabilities?.duplex ?? true;
  const canColor = capabilities?.color ?? true;
  const canCopies = capabilities?.copies ?? true;

  const valid =
    form.name.trim() !== "" && form.path.trim() !== "" && fileTypes.length > 0;

  function toggleChip(id: string): void {
    setForm((f) => ({
      ...f,
      chipIds: f.chipIds.includes(id)
        ? f.chipIds.filter((c) => c !== id)
        : [...f.chipIds, id],
    }));
  }

  async function browse(): Promise<void> {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") {
      setForm((f) => ({ ...f, path: picked }));
    }
  }

  function handleSubmit(e: FormEvent<HTMLFormElement>): void {
    e.preventDefault();
    if (!valid) return;
    onSubmit({
      folder: {
        name: form.name.trim(),
        path: form.path.trim(),
        poll_interval_secs: intervalSecs,
        file_types: fileTypes,
        printer_name: form.printerName,
        // A printer that cannot honour a setting is sent the neutral value, so
        // the stored configuration never claims something that will be ignored.
        copies: canCopies ? Math.max(1, Number.parseInt(form.copies, 10) || 1) : 1,
        duplex: canDuplex ? form.duplex : "simplex",
        color_mode: canColor ? form.colorMode : "mono",
        post_action: form.postAction,
      },
      printExisting: creating ? printExisting : false,
    });
  }

  const existingLabel =
    existingCount === null
      ? "Vorhandene Dateien jetzt mitdrucken"
      : `Vorhandene ${existingCount} Dateien jetzt mitdrucken`;

  return (
    <div className="modal-overlay" role="dialog" aria-modal="true" aria-label={
      creating ? "Ordner hinzufügen" : "Ordner bearbeiten"
    }>
      <form className="modal" onSubmit={handleSubmit}>
        <div className="modal-head">
          <h2>{creating ? "Ordner hinzufügen" : "Ordner bearbeiten"}</h2>
        </div>

        <div className="field">
          <label htmlFor="fd-name">Name</label>
          <input
            id="fd-name"
            className="input"
            value={form.name}
            placeholder="z. B. Scanner"
            onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
          />
        </div>

        <div className="field">
          <label htmlFor="fd-path">Ordner</label>
          <div className="row">
            <input
              id="fd-path"
              className="input"
              style={{ flex: 1 }}
              value={form.path}
              placeholder="C:\\Scans"
              onChange={(e) => setForm((f) => ({ ...f, path: e.target.value }))}
            />
            <button type="button" className="btn btn-quiet" onClick={() => void browse()}>
              Durchsuchen …
            </button>
          </div>
        </div>

        <div className="field">
          <label>Dateitypen</label>
          <div className="chip-row">
            {FILE_TYPE_CHIPS.map((chip) => (
              <button
                key={chip.id}
                type="button"
                className={`chip${form.chipIds.includes(chip.id) ? " on" : ""}`}
                aria-pressed={form.chipIds.includes(chip.id)}
                onClick={() => toggleChip(chip.id)}
              >
                {chip.label}
              </button>
            ))}
          </div>
        </div>

        <div className="field">
          <label htmlFor="fd-interval">Prüfintervall (Sekunden)</label>
          <input
            id="fd-interval"
            className="input"
            type="number"
            min={1}
            value={form.pollInterval}
            onChange={(e) => setForm((f) => ({ ...f, pollInterval: e.target.value }))}
          />
          <p className="helper" data-testid="interval-hint">
            {stabilityHint(intervalSecs)}
          </p>
        </div>

        <div className="field">
          <label htmlFor="fd-printer">Drucker</label>
          <select
            id="fd-printer"
            className="input"
            value={form.printerName}
            onChange={(e) => {
              setForm((f) => ({ ...f, printerName: e.target.value }));
              onPrinterChange(e.target.value);
            }}
          >
            {printers.map((p) => (
              <option key={p.name} value={p.name}>
                {p.is_default ? `${p.name} (Standard)` : p.name}
              </option>
            ))}
          </select>
        </div>

        <div className="row">
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-copies">Kopien</label>
            <input
              id="fd-copies"
              className="input"
              type="number"
              min={1}
              disabled={!canCopies}
              value={form.copies}
              onChange={(e) => setForm((f) => ({ ...f, copies: e.target.value }))}
            />
            {!canCopies && <p className="helper">{UNSUPPORTED}</p>}
          </div>

          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-duplex">Duplex</label>
            <select
              id="fd-duplex"
              className="input"
              disabled={!canDuplex}
              value={form.duplex}
              onChange={(e) =>
                setForm((f) => ({ ...f, duplex: e.target.value as DuplexMode }))
              }
            >
              <option value="simplex">Einseitig</option>
              <option value="long_edge">Duplex (lange Kante)</option>
              <option value="short_edge">Duplex (kurze Kante)</option>
            </select>
            {!canDuplex && <p className="helper">{UNSUPPORTED}</p>}
          </div>
        </div>

        <div className="row">
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-color">Farbe</label>
            <select
              id="fd-color"
              className="input"
              disabled={!canColor}
              value={form.colorMode}
              onChange={(e) =>
                setForm((f) => ({ ...f, colorMode: e.target.value as ColorMode }))
              }
            >
              <option value="mono">Schwarz-weiß</option>
              <option value="color">Farbe</option>
            </select>
            {!canColor && <p className="helper">{UNSUPPORTED}</p>}
          </div>

          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-post">Nach dem Druck</label>
            <select
              id="fd-post"
              className="input"
              value={form.postAction}
              onChange={(e) =>
                setForm((f) => ({ ...f, postAction: e.target.value as PostAction }))
              }
            >
              <option value="move">In Unterordner verschieben</option>
              <option value="keep">Liegen lassen</option>
              <option value="delete">Löschen</option>
            </select>
          </div>
        </div>

        {creating && (
          <div className="field">
            <label className="switch-label" htmlFor="fd-print-existing">
              <input
                id="fd-print-existing"
                type="checkbox"
                checked={printExisting}
                onChange={(e) => setPrintExisting(e.target.checked)}
              />
              <span>{existingLabel}</span>
            </label>
            <p className="helper">
              Standardmäßig gilt alles, was schon im Ordner liegt, als erledigt. Nur
              neu hinzukommende Dateien werden gedruckt.
            </p>
          </div>
        )}

        <div className="modal-actions">
          <button type="button" className="link-btn" onClick={onCancel}>
            Abbrechen
          </button>
          <button type="submit" className="btn" disabled={!valid || saving}>
            {saving ? "Speichere …" : creating ? "Anlegen" : "Speichern"}
          </button>
        </div>
      </form>
    </div>
  );
}
```

Note on the label queries in the test: the checkbox is wrapped by its `<label>` *and* carries a matching `htmlFor`, so `getByLabelText` resolves it whether the count has arrived or not.

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/components/FolderDialog.test.tsx`
Expected: PASS — 13 tests.

- [ ] **Step 5: Commit**

```bash
git add src/components/FolderDialog.tsx src/components/FolderDialog.test.tsx
git commit -m "feat: add folder dialog with capability-driven control disabling"
```

---

### Task 7: Ordner screen

**Files:**
- Create: `src/routes/Folders.tsx`
- Test: `src/routes/Folders.test.tsx`

**Interfaces:**
- Consumes: `lib/api::api`; `lib/events::{onFolderEvent, onJobEvent, onQueueEvent}`; `lib/format::folderActivity`; `components/FolderCard`; `components/FolderDialog` and `FolderDialogResult`; `@tauri-apps/plugin-opener`'s `revealItemInDir`.
- Produces: `routes/Folders` — `default function Folders(): JSX.Element`; the query keys `["folders"]`, `["jobs", onlyFailed, limit]`, `["status"]`, `["printers"]`, `["capabilities", printer]`.

This screen owns every mutation on the Ordner tab and holds the live queue-hold flag that `FolderCard` renders. Backend events invalidate queries rather than patching cached rows: the database is the single source of truth, and a refetch is cheaper than keeping a second copy of the state machine in TypeScript.

- [ ] **Step 1: Write the failing test**

Create `src/routes/Folders.test.tsx`:

```tsx
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import Folders from "./Folders";
import type { PrintJob, WatchFolder } from "../lib/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const FOLDERS: WatchFolder[] = [
  {
    id: 1,
    name: "Scanner",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 5,
    file_types: '["pdf"]',
    printer_name: "HP LaserJet",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    post_action: "move",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
  },
];

const JOBS: PrintJob[] = [];

function respond(cmd: string, args?: Record<string, unknown>): unknown {
  switch (cmd) {
    case "list_folders_cmd":
      return FOLDERS;
    case "list_jobs_cmd":
      return JOBS;
    case "list_printers_cmd":
      return [{ name: "HP LaserJet", is_default: true }];
    case "printer_capabilities_cmd":
      return { duplex: true, color: true, copies: true };
    case "get_status_cmd":
      return {
        active_folders: 3,
        printed_today: 12,
        waiting: 0,
        failed: 0,
        user_paused: false,
      };
    case "set_global_paused_cmd":
    case "set_folder_enabled_cmd":
    case "scan_now_cmd":
      return undefined;
    default:
      throw new Error(`unexpected command ${cmd} ${JSON.stringify(args)}`);
  }
}

function renderScreen() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <Folders />
    </QueryClientProvider>,
  );
}

describe("Folders", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) =>
      respond(cmd, args),
    );
  });

  it("renders the aggregate header from get_status_cmd", async () => {
    renderScreen();
    await waitFor(() =>
      expect(screen.getByTestId("aggregate")).toHaveTextContent(
        "3 aktiv · 12 heute gedruckt",
      ),
    );
  });

  it("renders one card per folder plus the add tile", async () => {
    renderScreen();
    await waitFor(() => expect(screen.getByText("Scanner")).toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Ordner hinzufügen" })).toBeInTheDocument();
  });

  it("sends the global pause switch straight to set_global_paused_cmd", async () => {
    renderScreen();
    const toggle = await screen.findByLabelText("Alles pausieren");
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_global_paused_cmd", { paused: true }),
    );
  });

  it("opens the dialog from the add tile", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Ordner hinzufügen" }));
    expect(await screen.findByRole("dialog", { name: "Ordner hinzufügen" })).toBeInTheDocument();
  });

  it("triggers a manual scan for a single folder", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Jetzt scannen" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("scan_now_cmd", { id: 1 }),
    );
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/routes/Folders.test.tsx`
Expected: FAIL — `Failed to resolve import "./Folders"`.

- [ ] **Step 3: Write the screen**

Create `src/routes/Folders.tsx`:

```tsx
import { useEffect, useState, type JSX } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { onFolderEvent, onJobEvent, onQueueEvent } from "../lib/events";
import { folderActivity } from "../lib/format";
import type { WatchFolder } from "../lib/types";
import FolderCard from "../components/FolderCard";
import FolderDialog, { type FolderDialogResult } from "../components/FolderDialog";
import "./screens.css";

const JOB_LIMIT = 300;

export default function Folders(): JSX.Element {
  const qc = useQueryClient();
  const [dialogFor, setDialogFor] = useState<WatchFolder | null | undefined>(undefined);
  const [capabilityPrinter, setCapabilityPrinter] = useState<string | null>(null);
  const [queueHeld, setQueueHeld] = useState(false);
  const [holdReason, setHoldReason] = useState<string | null>(null);
  const [nowMs, setNowMs] = useState(() => Date.now());

  const folders = useQuery({ queryKey: ["folders"], queryFn: api.listFolders });
  const jobs = useQuery({
    queryKey: ["jobs", false, JOB_LIMIT],
    queryFn: () => api.listJobs(false, JOB_LIMIT),
  });
  const status = useQuery({ queryKey: ["status"], queryFn: api.getStatus });
  const printers = useQuery({ queryKey: ["printers"], queryFn: api.listPrinters });

  const activePrinter =
    capabilityPrinter ??
    printers.data?.find((p) => p.is_default)?.name ??
    printers.data?.[0]?.name ??
    null;

  const capabilities = useQuery({
    queryKey: ["capabilities", activePrinter],
    queryFn: () => api.printerCapabilities(activePrinter as string),
    enabled: activePrinter !== null,
  });

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

  const save = useMutation({
    mutationFn: (vars: { id: number | null; result: FolderDialogResult }) =>
      vars.id === null
        ? api.createFolder(vars.result.folder, vars.result.printExisting)
        : api.updateFolder(vars.id, vars.result.folder),
    onSuccess: () => {
      invalidateAll();
      setDialogFor(undefined);
    },
  });

  const userPaused = status.data?.user_paused ?? false;
  const busy = toggleFolder.isPending || scanNow.isPending;

  return (
    <section className="screen">
      <div className="screen-header">
        <h1>Ordner</h1>
        <button type="button" className="btn" onClick={() => setDialogFor(null)}>
          Ordner hinzufügen
        </button>
      </div>

      <div className="aggregate">
        <span data-testid="aggregate">
          {status.data === undefined
            ? "Lade …"
            : `${status.data.active_folders} aktiv · ${status.data.printed_today} heute gedruckt`}
        </span>
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
              onEdit={(folder) => {
                setCapabilityPrinter(folder.printer_name);
                setDialogFor(folder);
              }}
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
        onClick={() => setDialogFor(null)}
      >
        + Ordner hinzufügen
      </button>

      {dialogFor !== undefined && (
        <FolderDialog
          folder={dialogFor}
          printers={printers.data ?? []}
          capabilities={capabilities.data ?? null}
          countExisting={(path, fileTypes) => api.countExistingFiles(path, fileTypes)}
          onPrinterChange={setCapabilityPrinter}
          onCancel={() => setDialogFor(undefined)}
          onSubmit={(result) =>
            save.mutate({ id: dialogFor === null ? null : dialogFor.id, result })
          }
          saving={save.isPending}
        />
      )}
    </section>
  );
}
```

Note: the header button and the dashed tile carry different accessible names — "Ordner hinzufügen" and "+ Ordner hinzufügen" — so `getByRole("button", { name: "Ordner hinzufügen" })` stays unambiguous.

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/routes/Folders.test.tsx`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src/routes/Folders.tsx src/routes/Folders.test.tsx
git commit -m "feat: add ordner screen with aggregate status and global pause"
```

---

### Task 8: Verlauf screen

**Files:**
- Create: `src/routes/History.tsx`
- Test: `src/routes/History.test.tsx`

**Interfaces:**
- Consumes: `lib/api::api`; `lib/events::onJobEvent`; `lib/format::{formatDateTime, jobOutcomeLabel}`; `lib/types::{PrintJob, WatchFolder}`.
- Produces: `routes/History` — `default function History(): JSX.Element`.

The filter uses the backend's own `only_failed` parameter rather than filtering a cached array, so the "Nur Fehler" view stays correct past the row limit.

- [ ] **Step 1: Write the failing test**

Create `src/routes/History.test.tsx`:

```tsx
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import History from "./History";
import type { PrintJob, WatchFolder } from "../lib/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

function job(overrides: Partial<PrintJob>): PrintJob {
  return {
    id: 1,
    folder_id: 1,
    file_path: "/Users/tim/Scans/a.pdf",
    file_name: "a.pdf",
    size_bytes: 10,
    mtime_ms: 1,
    sha256: "h",
    state: "done",
    attempts: 0,
    printer_name: "HP LaserJet",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    error_kind: null,
    error_message: null,
    next_attempt_at: null,
    enqueued_at: "2026-08-12 09:00:00",
    started_at: "2026-08-12 09:00:01",
    finished_at: "2026-08-12 09:00:05",
    ...overrides,
  };
}

const JOBS: PrintJob[] = [
  job({ id: 1, file_name: "rechnung.pdf", state: "done" }),
  job({
    id: 2,
    file_name: "kaputt.pdf",
    state: "failed",
    error_kind: "file",
    error_message: "PDF nicht lesbar",
  }),
];

const FOLDERS: WatchFolder[] = [
  {
    id: 1,
    name: "Scanner",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 5,
    file_types: '["pdf"]',
    printer_name: "HP LaserJet",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    post_action: "move",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
  },
];

function renderScreen() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <History />
    </QueryClientProvider>,
  );
}

describe("History", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "list_jobs_cmd") {
        return args?.onlyFailed === true
          ? JOBS.filter((j) => j.state === "failed")
          : JOBS;
      }
      if (cmd === "list_folders_cmd") return FOLDERS;
      if (cmd === "reprint_job_cmd") return undefined;
      throw new Error(`unexpected command ${cmd}`);
    });
  });

  it("lists file, folder, printer, time and outcome", async () => {
    renderScreen();
    expect(await screen.findByText("rechnung.pdf")).toBeInTheDocument();
    const row = screen.getByTestId("job-row-1");
    expect(row).toHaveTextContent("Scanner");
    expect(row).toHaveTextContent("HP LaserJet");
    expect(row).toHaveTextContent("12.08.2026");
    expect(row).toHaveTextContent("Gedruckt");
  });

  it("filters to failures only and back", async () => {
    renderScreen();
    expect(await screen.findByText("rechnung.pdf")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Nur Fehler"));
    await waitFor(() =>
      expect(screen.queryByText("rechnung.pdf")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("kaputt.pdf")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("list_jobs_cmd", {
      onlyFailed: true,
      limit: 300,
    });

    fireEvent.click(screen.getByLabelText("Nur Fehler"));
    await waitFor(() => expect(screen.getByText("rechnung.pdf")).toBeInTheDocument());
  });

  it("shows the error message on a failed job", async () => {
    renderScreen();
    expect(await screen.findByText("PDF nicht lesbar")).toBeInTheDocument();
  });

  it("reprints a single row", async () => {
    renderScreen();
    await screen.findByText("rechnung.pdf");
    fireEvent.click(screen.getAllByRole("button", { name: "Erneut drucken" })[0]);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("reprint_job_cmd", { id: 1 }),
    );
  });

  it("shows an empty state when nothing has been printed yet", async () => {
    invokeMock.mockImplementation(async (cmd: string) =>
      cmd === "list_jobs_cmd" ? [] : FOLDERS,
    );
    renderScreen();
    expect(await screen.findByText("Noch nichts gedruckt")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/routes/History.test.tsx`
Expected: FAIL — `Failed to resolve import "./History"`.

- [ ] **Step 3: Write the screen**

Create `src/routes/History.tsx`:

```tsx
import { useEffect, useState, type JSX } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";
import { onJobEvent } from "../lib/events";
import { formatDateTime, jobOutcomeLabel } from "../lib/format";
import "./screens.css";

const JOB_LIMIT = 300;

export default function History(): JSX.Element {
  const qc = useQueryClient();
  const [onlyFailed, setOnlyFailed] = useState(false);

  const jobs = useQuery({
    queryKey: ["jobs", onlyFailed, JOB_LIMIT],
    queryFn: () => api.listJobs(onlyFailed, JOB_LIMIT),
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

  const reprint = useMutation({
    mutationFn: (id: number) => api.reprintJob(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["jobs"] });
      void qc.invalidateQueries({ queryKey: ["status"] });
    },
  });

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
        „Gedruckt" heißt: Der Auftrag wurde an den Drucker übergeben. Ob wirklich
        Papier herauskam, kann Printy nicht sehen.
      </p>

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
                <button
                  type="button"
                  className="link-btn"
                  disabled={reprint.isPending}
                  onClick={() => reprint.mutate(j.id)}
                >
                  Erneut drucken
                </button>
              </div>
            </li>
          ))}
        </ul>
      ) : (
        <div className="card empty">
          <p className="empty-title">
            {onlyFailed ? "Keine Fehler" : "Noch nichts gedruckt"}
          </p>
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
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/routes/History.test.tsx`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src/routes/History.tsx src/routes/History.test.tsx
git commit -m "feat: add verlauf screen with failure filter and per-row reprint"
```

---

### Task 9: Einstellungen and Über screens

**Files:**
- Create: `src/routes/Settings.tsx`
- Create: `src/routes/About.tsx`
- Create: `scripts/gen-licenses.sh`
- Create: `src/assets/third-party-licenses.json` (generated)
- Test: `src/routes/Settings.test.tsx`

**Interfaces:**
- Consumes: `lib/api::api`; `lib/theme::{ThemeChoice, applyTheme, getThemeChoice}`; `@tauri-apps/api/path`'s `appDataDir` and `join`; `@tauri-apps/plugin-opener`'s `revealItemInDir` and `openUrl`.
- Produces: `routes/Settings` — `default function Settings(): JSX.Element`; `routes/About` — `default function About(): JSX.Element`.

The theme is stored twice on purpose: `localStorage` is authoritative at startup, because the theme has to be applied before the first paint and `get_settings_cmd` is asynchronous; the `theme` key in the database is kept in sync so the setting is not lost with the browser storage. Everything else is a straight `update_setting_cmd`, except autostart, which goes through Task 3's command so the OS login item changes immediately.

- [ ] **Step 1: Write the failing test**

Create `src/routes/Settings.test.tsx`:

```tsx
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
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_settings_cmd") {
        return {
          notification_mode: "all",
          autostart: "0",
          start_minimized: "0",
          theme: "system",
          sumatra_path: "",
          user_paused: "0",
        };
      }
      if (cmd === "get_autostart_cmd") return false;
      return undefined;
    });
  });

  it("writes the notification mode through update_setting_cmd", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Nur Fehler" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "notification_mode",
        value: "errors",
      }),
    );
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

  it("applies the theme locally and mirrors it into the database", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Dunkel" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
        key: "theme",
        value: "dark",
      }),
    );
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/routes/Settings.test.tsx`
Expected: FAIL — `Failed to resolve import "./Settings"`.

- [ ] **Step 3: Write the Einstellungen screen**

Create `src/routes/Settings.tsx`:

```tsx
import { useEffect, useState, type JSX } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { appDataDir, join } from "@tauri-apps/api/path";
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

  const settings = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const autostart = useQuery({ queryKey: ["autostart"], queryFn: api.getAutostart });

  useEffect(() => {
    if (settings.data !== undefined) setSumatra(settings.data.sumatra_path);
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
    // localStorage is authoritative at startup — the theme must land before the
    // first paint, and the settings command is asynchronous. The database copy
    // keeps the choice if browser storage is ever cleared.
    applyTheme(choice);
    setTheme(choice);
    updateSetting.mutate({ key: "theme", value: choice });
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
              onClick={() =>
                updateSetting.mutate({ key: "notification_mode", value: opt.value })
              }
            >
              {opt.label}
            </button>
          ))}
        </div>
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
            onChange={(e) => setSumatra(e.target.value)}
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
```

- [ ] **Step 4: Write the licence generator and the Über screen**

Create `scripts/gen-licenses.sh`, following tabsy's script verbatim in approach — Rust crates from `cargo license`, npm packages from `license-checker-rseidelsohn`, merged and deduplicated into a JSON asset:

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p src/assets
( cd src-tauri && (cargo install cargo-license --quiet 2>/dev/null || true) && cargo license --json ) > /tmp/printy-rust-lic.json || echo "[]" > /tmp/printy-rust-lic.json
npx --yes license-checker-rseidelsohn --production --json > /tmp/printy-npm-lic.json || echo "{}" > /tmp/printy-npm-lic.json
node -e '
const fs=require("fs");
let rust=[]; try{rust=JSON.parse(fs.readFileSync("/tmp/printy-rust-lic.json","utf8")).map(c=>({name:c.name,version:c.version,license:(c.license||c.license_file||"unknown").toString()}));}catch(e){}
let npm=[]; try{const r=JSON.parse(fs.readFileSync("/tmp/printy-npm-lic.json","utf8"));npm=Object.entries(r).map(([k,v])=>{const i=k.lastIndexOf("@");return {name:k.slice(0,i)||k,version:k.slice(i+1),license:(v.licenses||"unknown").toString()};});}catch(e){}
const all=[...rust,...npm].filter(x=>x.name);
const seen=new Set(),out=[];
for(const x of all.sort((a,b)=>a.name.localeCompare(b.name))){const k=x.name+"@"+x.version;if(!seen.has(k)){seen.add(k);out.push(x);}}
fs.writeFileSync("src/assets/third-party-licenses.json",JSON.stringify(out,null,2));
console.log("wrote "+out.length+" entries");
'
```

Then:

```bash
chmod +x scripts/gen-licenses.sh
mkdir -p src/assets && printf '[]' > src/assets/third-party-licenses.json
./scripts/gen-licenses.sh
```

If the generator cannot reach the network the empty `[]` stays in place; the screen handles that case explicitly.

Create `src/routes/About.tsx`:

```tsx
import { useMemo, useState, type JSX } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import pkg from "../../package.json";
import licensesData from "../assets/third-party-licenses.json";
import Logo from "../components/Logo";
import "./screens.css";

interface LicenseEntry {
  name: string;
  version: string;
  license: string;
}

const licenses: LicenseEntry[] = licensesData as LicenseEntry[];
const version: string = pkg.version;

export default function About(): JSX.Element {
  const [filter, setFilter] = useState("");

  const filtered = useMemo<LicenseEntry[]>(() => {
    const q = filter.trim().toLowerCase();
    if (q.length === 0) return licenses;
    return licenses.filter((entry) => entry.name.toLowerCase().includes(q));
  }, [filter]);

  return (
    <section className="screen">
      <h1>Über Printy</h1>

      <div className="card">
        <Logo size={44} wordmark />
        <p style={{ marginTop: "0.9rem" }}>
          Legt Dateien aus überwachten Ordnern automatisch auf den Drucker.
        </p>
        <p className="helper">Version {version}</p>
        <p style={{ marginTop: "0.25rem" }}>
          von{" "}
          <button
            type="button"
            className="link-btn"
            onClick={() => {
              void openUrl("https://noix.dev");
            }}
          >
            noix.dev
          </button>
        </p>
      </div>

      <div className="card">
        <h2>Lizenzen</h2>
        {licenses.length === 0 ? (
          <p className="helper">Lizenzliste wird beim nächsten Build erzeugt.</p>
        ) : (
          <>
            <div className="field">
              <input
                className="input"
                type="text"
                value={filter}
                placeholder="Nach Name filtern …"
                aria-label="Lizenzen nach Name filtern"
                onChange={(e) => setFilter(e.target.value)}
              />
            </div>
            <p className="helper" style={{ marginTop: 0 }}>
              {filtered.length} von {licenses.length} Einträgen
            </p>
            <ul
              style={{
                listStyle: "none",
                margin: "0.5rem 0 0",
                padding: 0,
                maxHeight: "22rem",
                overflowY: "auto",
              }}
            >
              {filtered.map((entry) => (
                <li
                  key={`${entry.name}@${entry.version}`}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "0.5rem",
                    flexWrap: "wrap",
                    padding: "0.45rem 0",
                    borderBottom: "1px solid var(--ink-08)",
                  }}
                >
                  <span style={{ fontWeight: 600 }}>{entry.name}</span>
                  <span className="helper" style={{ margin: 0 }}>
                    · {entry.version} ·
                  </span>
                  <span className="badge">{entry.license}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `npx vitest run src/routes/Settings.test.tsx`
Expected: PASS — 6 tests.

- [ ] **Step 6: Commit**

```bash
git add src/routes/Settings.tsx src/routes/Settings.test.tsx src/routes/About.tsx \
  src/assets/third-party-licenses.json scripts/gen-licenses.sh
git commit -m "feat: add einstellungen and über screens with generated licence list"
```

---

### Task 10: App shell, sidebar and routing

**Files:**
- Create: `src/App.tsx`
- Create: `src/App.css`
- Test: `src/App.test.tsx`
- Modify: `src/main.tsx` (replace the scaffold's contents entirely)

**Interfaces:**
- Consumes: `components/Logo`; `routes/Folders`; `routes/History`; `routes/Settings`; `routes/About`; `lib/theme::initTheme`.
- Produces: `App` — `default function App(): JSX.Element`; the routes `/`, `/history`, `/settings`, `/about`; the shell classes `.app-shell`, `.sidebar`, `.nav-link`.

The shell is tabsy's, with the accent rail and the active nav item recoloured from mint to coral — mint is reserved for running and printed.

- [ ] **Step 1: Write the failing test**

Create `src/App.test.tsx`:

```tsx
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

describe("App shell", () => {
  it("renders the brand lockup in the sidebar", () => {
    renderApp();
    expect(screen.getByText("Printy")).toBeInTheDocument();
  });

  it("offers the four navigation entries in order", () => {
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/App.test.tsx`
Expected: FAIL — `Failed to resolve import "./App"` (the scaffold's `App.tsx` is replaced, not extended).

- [ ] **Step 3: Write the shell**

Replace `src/App.tsx` entirely:

```tsx
import type { JSX, ReactNode } from "react";
import { BrowserRouter, NavLink, Route, Routes } from "react-router-dom";
import Folders from "./routes/Folders";
import History from "./routes/History";
import Settings from "./routes/Settings";
import About from "./routes/About";
import Logo from "./components/Logo";
import "./App.css";

interface NavItem {
  to: string;
  label: string;
  icon: ReactNode;
  end?: boolean;
}

const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Ordner", icon: "📁", end: true },
  { to: "/history", label: "Verlauf", icon: "🧾" },
  { to: "/settings", label: "Einstellungen", icon: "⚙️" },
  { to: "/about", label: "Über", icon: "ℹ️" },
];

function Sidebar(): JSX.Element {
  return (
    <aside className="sidebar">
      <div className="brand">
        <Logo size={30} wordmark wordmarkColor="#f4f1ea" />
      </div>
      <nav className="nav" aria-label="Hauptnavigation">
        {NAV_ITEMS.map((item) => (
          <NavLink key={item.to} to={item.to} end={item.end} className="nav-link">
            <span className="nav-icon" aria-hidden="true">
              {item.icon}
            </span>
            <span className="nav-label">{item.label}</span>
          </NavLink>
        ))}
      </nav>
      <div className="sidebar-foot">Ordner rein, Papier raus.</div>
    </aside>
  );
}

function App(): JSX.Element {
  return (
    <BrowserRouter>
      <div className="app-shell">
        <Sidebar />
        <main className="content">
          <Routes>
            <Route path="/" element={<Folders />} />
            <Route path="/history" element={<History />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/about" element={<About />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}

export default App;
```

Replace `src/App.css` entirely — tabsy's shell with the accent rail switched to coral:

```css
.app-shell {
  display: grid;
  grid-template-columns: 240px 1fr;
  min-height: 100vh;
}

/* Sidebar */
.sidebar {
  background: var(--teal-grad);
  color: #f4f1ea;
  display: flex;
  flex-direction: column;
  gap: 26px;
  padding: 26px 16px 20px;
  position: sticky;
  top: 0;
  height: 100vh;
  border-right: 1px solid rgba(255, 255, 255, 0.04);
}

.brand {
  display: flex;
  align-items: center;
  padding: 4px 10px 6px;
}

/* Navigation */
.nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-link {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 11px 14px;
  border-radius: var(--radius-sm);
  text-decoration: none;
  font-size: 15px;
  font-weight: 600;
  color: rgba(244, 241, 234, 0.62);
  transition: background 0.16s ease, color 0.16s ease, transform 0.16s ease;
}

.nav-icon {
  font-size: 16px;
  line-height: 1;
  filter: grayscale(0.2);
  opacity: 0.85;
  transition: filter 0.16s ease, opacity 0.16s ease;
}

.nav-link:hover {
  background: rgba(255, 255, 255, 0.06);
  color: #f4f1ea;
}

.nav-link:hover .nav-icon {
  filter: grayscale(0);
  opacity: 1;
}

.nav-link.active {
  background: var(--coral-soft);
  color: var(--coral);
}

.nav-link.active .nav-icon {
  filter: grayscale(0);
  opacity: 1;
}

/* Coral accent rail on the active item */
.nav-link.active::before {
  content: "";
  position: absolute;
  left: -16px;
  top: 50%;
  transform: translateY(-50%);
  width: 4px;
  height: 22px;
  border-radius: 0 4px 4px 0;
  background: var(--coral);
}

.sidebar-foot {
  margin-top: auto;
  padding: 0 12px;
  font-size: 12px;
  line-height: 1.4;
  color: rgba(244, 241, 234, 0.4);
}

/* Content */
.content {
  background: transparent;
  color: var(--ink);
  padding: 40px 40px 64px;
  overflow-y: auto;
}
```

Replace `src/main.tsx` entirely:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { initTheme } from "./lib/theme";
import "@fontsource-variable/fraunces";
import "./theme.css";

initTheme();

const queryClient = new QueryClient();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
```

Delete the scaffold leftovers that nothing imports any more:

```bash
rm -f src/assets/react.svg
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/App.test.tsx`
Expected: PASS — 4 tests.

Run: `npm test`
Expected: PASS — every suite from Tasks 1, 2, 4, 5, 6, 7, 8, 9 and 10.

Run: `npx tsc --noEmit`
Expected: no type errors.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/App.css src/App.test.tsx src/main.tsx
git rm --cached -q src/assets/react.svg 2>/dev/null || true
git commit -m "feat: add sidebar app shell with ordner, verlauf, einstellungen and über routes"
```

---

### Task 11: Tray icon, close-to-tray and start minimised

**Files:**
- Create: `src-tauri/src/shell/mod.rs`
- Create: `src-tauri/src/shell/tray.rs`
- Modify: `src-tauri/src/lib.rs` (four insertions, anchors given in Step 4)
- Modify: `src-tauri/tauri.conf.json` (window `visible: false`)

**Interfaces:**
- Consumes: `db::settings::{get_setting, set_setting, user_paused, start_minimized}`; `AppState`; the event topic `printy://queue`.
- Produces: `shell::tray::{TRAY_ID, TrayState, tray_state, tray_rgba, setup_tray, spawn_tray_updater, show_main, toggle_main}`.

This is Rust work extending plan 1's `lib.rs`, modelled on tabsy's tray setup at `tabs-manager/src-tauri/src/lib.rs:64-118`. The icon is drawn programmatically as raw RGBA rather than loaded from a file: `Image::new_owned` needs no extra cargo feature, the colour is a single constant per state, and the drawing is a pure function that can be unit-tested. The printer hold is not readable from the database — it is deliberately runtime-only state — so the updater listens to `printy://queue` and keeps its own flag, exactly the signal the queue scheduler already emits.

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/shell/tray.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_users_pause_outranks_everything_else() {
        assert_eq!(tray_state(true, true, 5, 5), TrayState::Paused);
        assert_eq!(tray_state(true, false, 0, 0), TrayState::Paused);
    }

    #[test]
    fn a_held_queue_or_a_broken_folder_or_a_recent_failure_shows_an_error() {
        assert_eq!(tray_state(false, true, 0, 0), TrayState::Error);
        assert_eq!(tray_state(false, false, 0, 1), TrayState::Error);
        assert_eq!(tray_state(false, false, 1, 0), TrayState::Error);
    }

    #[test]
    fn everything_healthy_and_running_shows_mint() {
        assert_eq!(tray_state(false, false, 0, 0), TrayState::Running);
    }

    #[test]
    fn each_state_carries_its_own_brand_colour() {
        assert_eq!(TrayState::Running.rgb(), [0x2f, 0xe6, 0xb7]);
        assert_eq!(TrayState::Error.rgb(), [0xff, 0x7a, 0x59]);
        assert_eq!(TrayState::Paused.rgb(), [0x9a, 0xa3, 0xa1]);
    }

    #[test]
    fn the_icon_is_a_full_rgba_buffer_of_the_declared_size() {
        let px = tray_rgba(TrayState::Running);
        assert_eq!(px.len() as u32, ICON_SIZE * ICON_SIZE * 4);
    }

    #[test]
    fn the_folder_body_is_opaque_and_the_corners_are_transparent() {
        let px = tray_rgba(TrayState::Error);
        let at = |x: u32, y: u32| {
            let i = ((y * ICON_SIZE + x) * 4) as usize;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        assert_eq!(at(16, 18), [0xff, 0x7a, 0x59, 0xff]);
        assert_eq!(at(0, 0)[3], 0);
        assert_eq!(at(31, 31)[3], 0);
    }

    #[test]
    fn the_tab_above_the_body_is_drawn_too() {
        let px = tray_rgba(TrayState::Paused);
        let i = ((8 * ICON_SIZE + 6) * 4) as usize;
        assert_eq!(px[i + 3], 0xff);
    }

    #[test]
    fn different_states_produce_different_pixels() {
        assert_ne!(tray_rgba(TrayState::Running), tray_rgba(TrayState::Paused));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test shell::tray`
Expected: FAIL — `file not found for module 'shell'`.

- [ ] **Step 3: Write the tray module**

Create `src-tauri/src/shell/mod.rs`:

```rust
pub mod tray;
```

Prepend to `src-tauri/src/shell/tray.rs`:

```rust
use crate::db::settings;
use crate::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager};

pub const TRAY_ID: &str = "printy-tray";
const ICON_SIZE: u32 = 32;
const UPDATE_INTERVAL_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Running,
    Error,
    Paused,
}

impl TrayState {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            // Mint means running, coral means something needs attention, grey
            // means the user switched it off. Same three roles as the folder dots.
            TrayState::Running => [0x2f, 0xe6, 0xb7],
            TrayState::Error => [0xff, 0x7a, 0x59],
            TrayState::Paused => [0x9a, 0xa3, 0xa1],
        }
    }
}

/// The user's own pause outranks everything: a deliberately stopped Printy must
/// not shout. A held queue counts as an error even though no job failed —
/// otherwise a switched-off printer would leave the tray cheerfully mint.
pub fn tray_state(
    user_paused: bool,
    queue_held: bool,
    recent_failures: i64,
    folder_errors: i64,
) -> TrayState {
    if user_paused {
        return TrayState::Paused;
    }
    if queue_held || recent_failures > 0 || folder_errors > 0 {
        return TrayState::Error;
    }
    TrayState::Running
}

/// Draws the Printy folder silhouette as raw RGBA. Generated rather than loaded
/// so the state colour is a constant, not three binary assets, and so the shape
/// is testable.
pub fn tray_rgba(state: TrayState) -> Vec<u8> {
    let [r, g, b] = state.rgb();
    let mut px = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let in_tab = (3..14).contains(&x) && (7..11).contains(&y);
            let in_body = (3..29).contains(&x) && (10..26).contains(&y);
            if !in_tab && !in_body {
                continue;
            }
            let i = ((y * ICON_SIZE + x) * 4) as usize;
            px[i] = r;
            px[i + 1] = g;
            px[i + 2] = b;
            px[i + 3] = 0xff;
        }
    }
    px
}

fn icon_for(state: TrayState) -> Image<'static> {
    Image::new_owned(tray_rgba(state), ICON_SIZE, ICON_SIZE)
}

/// Shows the main window and brings it to the front.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Toggles the main window's visibility (for the tray icon click).
pub fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open_i = MenuItem::with_id(app, "open", "Öffnen", true, None::<&str>)?;
    let pause_i = MenuItem::with_id(app, "pause", "Pause", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_i, &pause_i, &quit_i])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon_for(TrayState::Paused))
        .tooltip("Printy")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "pause" => {
                // Flip the persisted global switch. Cloned out of State before
                // the await, because the guard is not Send.
                let handle = app.clone();
                let db = { handle.state::<AppState>().db.clone() };
                tauri::async_runtime::spawn(async move {
                    let now = settings::user_paused(&db).await;
                    let _ = settings::set_setting(
                        &db,
                        "user_paused",
                        if now { "0" } else { "1" },
                    )
                    .await;
                    let _ = handle.emit(
                        "printy://queue",
                        serde_json::json!({ "held": false, "paused": !now }),
                    );
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Keeps the tray colour in step with reality. The printer hold is runtime-only
/// state that never reaches the database, so it is picked up from the very event
/// the queue scheduler already emits.
pub fn spawn_tray_updater(app: AppHandle) {
    let held = Arc::new(AtomicBool::new(false));

    let held_for_listener = held.clone();
    app.listen("printy://queue", move |event| {
        let is_held = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("held").and_then(|h| h.as_bool()))
            .unwrap_or(false);
        held_for_listener.store(is_held, Ordering::Relaxed);
    });

    tauri::async_runtime::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_millis(UPDATE_INTERVAL_MS));
        let mut last: Option<TrayState> = None;

        loop {
            ticker.tick().await;
            let db = {
                let state = app.state::<AppState>();
                state.db.clone()
            };

            let paused = settings::user_paused(&db).await;
            // Only recent failures colour the tray. An error from last week must
            // not leave it permanently coral.
            let recent_failures: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM print_job
                 WHERE state = 'failed' AND finished_at >= datetime('now', '-1 hour')",
            )
            .fetch_one(&db)
            .await
            .unwrap_or(0);
            let folder_errors: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM watch_folder WHERE status != 'ok'")
                    .fetch_one(&db)
                    .await
                    .unwrap_or(0);

            let next = tray_state(
                paused,
                held.load(Ordering::Relaxed),
                recent_failures,
                folder_errors,
            );
            if last == Some(next) {
                continue;
            }
            last = Some(next);
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let _ = tray.set_icon(Some(icon_for(next)));
            }
        }
    });
}
```

- [ ] **Step 4: Wire the shell into `lib.rs`**

Four insertions in `src-tauri/src/lib.rs`, on top of the file plan 1 leaves behind after its Task 17.

*Insertion 1* — add the module declaration after `mod queue;`, keeping the list alphabetical:

```rust
mod shell;
```

*Insertion 2* — inside the `.setup(|app| { … })` closure, immediately after
`queue::scheduler::spawn_queue_worker(app.handle().clone());` and before `Ok(())`:

```rust
            shell::tray::setup_tray(app)?;
            shell::tray::spawn_tray_updater(app.handle().clone());

            // The window starts hidden (tauri.conf.json), so autostart lands in
            // the tray. Show it unless the user asked for a minimised start.
            let start_min = tauri::async_runtime::block_on(db::settings::start_minimized(
                &app.state::<AppState>().db,
            ));
            if !start_min {
                shell::tray::show_main(app.handle());
            }
```

*Insertion 3* — between the closing `})` of `.setup(...)` and the `.invoke_handler(...)` call:

```rust
        .on_window_event(|window, event| {
            // Closing puts Printy in the tray instead of quitting it — the whole
            // point of the app is that it keeps watching.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
```

*Insertion 4* — in `src-tauri/tauri.conf.json`, make the window start hidden so a minimised autostart never flashes a window:

```json
      {
        "title": "Printy",
        "width": 900,
        "height": 680,
        "visible": false
      }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test shell::tray`
Expected: PASS — 8 tests.

Run: `cd src-tauri && cargo test`
Expected: PASS — every test from plan 1 plus Tasks 3 and 11.

Run: `npm run tauri dev`
Expected: no window appears on a first run only if "Minimiert starten" is set; otherwise the window opens. A tray icon is present; its menu offers Öffnen, Pause and Beenden; closing the window hides it and the tray icon stays.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/shell src-tauri/src/lib.rs src-tauri/tauri.conf.json
git commit -m "feat: add state-coloured tray icon, close-to-tray and minimised start"
```

---

### Task 12: Desktop notifications

**Files:**
- Create: `src/lib/notify.ts`
- Create: `src/lib/useJobNotifications.ts`
- Test: `src/lib/notify.test.ts`
- Modify: `src/App.tsx` (one hook call inside `App`)

**Interfaces:**
- Consumes: `lib/api::api`; `lib/events::{onJobEvent, onQueueEvent}`; `lib/types::{JobOutcome, NotificationMode, PrintJob}`; `@tauri-apps/plugin-notification`'s `isPermissionGranted`, `requestPermission`, `sendNotification`.
- Produces: `lib/notify::{shouldNotify, jobNotification, queueNotification, type Toast}`; `lib/useJobNotifications::useJobNotifications`.

`printy://job` carries only the outcome — the debug name of `queue::worker::QueueOutcome` — so the file name has to be read back from the job ledger. For a failure that means the newest row from `list_jobs_cmd` with `only_failed = true`; for a success, the newest `done` row in the unfiltered page. Guessing from the plain newest row would name the wrong file whenever a job was enqueued in between.

- [ ] **Step 1: Write the failing test**

Create `src/lib/notify.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { jobNotification, queueNotification, shouldNotify } from "./notify";
import type { PrintJob } from "./types";

function job(overrides: Partial<PrintJob> = {}): PrintJob {
  return {
    id: 1,
    folder_id: 1,
    file_path: "/Users/tim/Scans/rechnung.pdf",
    file_name: "rechnung.pdf",
    size_bytes: 10,
    mtime_ms: 1,
    sha256: "h",
    state: "done",
    attempts: 0,
    printer_name: "HP LaserJet",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    error_kind: null,
    error_message: null,
    next_attempt_at: null,
    enqueued_at: "2026-08-12 09:00:00",
    started_at: "2026-08-12 09:00:01",
    finished_at: "2026-08-12 09:00:05",
    ...overrides,
  };
}

describe("shouldNotify", () => {
  it("says nothing at all when notifications are off", () => {
    expect(shouldNotify("off", "Printed")).toBe(false);
    expect(shouldNotify("off", "Failed")).toBe(false);
  });

  it("reports only failures in errors mode", () => {
    expect(shouldNotify("errors", "Printed")).toBe(false);
    expect(shouldNotify("errors", "Failed")).toBe(true);
  });

  it("reports prints and failures in all mode", () => {
    expect(shouldNotify("all", "Printed")).toBe(true);
    expect(shouldNotify("all", "Failed")).toBe(true);
  });

  it("never reports a retry — it is not an outcome the user must act on", () => {
    expect(shouldNotify("all", "Retried")).toBe(false);
    expect(shouldNotify("errors", "Retried")).toBe(false);
  });
});

describe("jobNotification", () => {
  it("names the file and the printer on success", () => {
    expect(jobNotification("Printed", job())).toEqual({
      title: "Gedruckt",
      body: "rechnung.pdf → HP LaserJet",
    });
  });

  it("names the file and the reason on failure", () => {
    expect(
      jobNotification("Failed", job({ state: "failed", error_message: "PDF nicht lesbar" })),
    ).toEqual({
      title: "Druck fehlgeschlagen",
      body: "rechnung.pdf: PDF nicht lesbar",
    });
  });

  it("falls back to a generic reason when the backend gave none", () => {
    expect(jobNotification("Failed", job({ state: "failed" }))).toEqual({
      title: "Druck fehlgeschlagen",
      body: "rechnung.pdf: Unbekannter Fehler",
    });
  });

  it("produces nothing for a retry or for a missing job", () => {
    expect(jobNotification("Retried", job())).toBeNull();
    expect(jobNotification("Printed", null)).toBeNull();
  });
});

describe("queueNotification", () => {
  it("announces a hold with its reason", () => {
    expect(queueNotification({ held: true, reason: "Drucker offline" })).toEqual({
      title: "Warteschlange angehalten",
      body: "Drucker offline. Printy versucht es jede Minute erneut.",
    });
  });

  it("says nothing when the queue resumes", () => {
    expect(queueNotification({ held: false })).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/lib/notify.test.ts`
Expected: FAIL — `Failed to resolve import "./notify"`.

- [ ] **Step 3: Write the notification rules**

Create `src/lib/notify.ts`:

```ts
import type { JobOutcome, NotificationMode, PrintJob, QueueEvent } from "./types";

export interface Toast {
  title: string;
  body: string;
}

/**
 * A retry is never announced. It is a transient step the queue takes on its own,
 * and three toasts on the way to one failure would train the user to ignore them.
 */
export function shouldNotify(mode: NotificationMode, outcome: JobOutcome): boolean {
  if (outcome === "Retried") return false;
  switch (mode) {
    case "off":
      return false;
    case "errors":
      return outcome === "Failed";
    case "all":
      return true;
  }
}

export function jobNotification(outcome: JobOutcome, job: PrintJob | null): Toast | null {
  if (job === null) return null;
  if (outcome === "Printed") {
    return { title: "Gedruckt", body: `${job.file_name} → ${job.printer_name}` };
  }
  if (outcome === "Failed") {
    return {
      title: "Druck fehlgeschlagen",
      body: `${job.file_name}: ${job.error_message ?? "Unbekannter Fehler"}`,
    };
  }
  return null;
}

export function queueNotification(event: QueueEvent): Toast | null {
  if (!event.held) return null;
  return {
    title: "Warteschlange angehalten",
    body: `${event.reason ?? "Drucker nicht erreichbar"}. Printy versucht es jede Minute erneut.`,
  };
}
```

- [ ] **Step 4: Write the hook**

Create `src/lib/useJobNotifications.ts`:

```ts
import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { api } from "./api";
import { onJobEvent, onQueueEvent } from "./events";
import { jobNotification, queueNotification, shouldNotify, type Toast } from "./notify";
import type { JobOutcome, NotificationMode, PrintJob } from "./types";

async function deliver(toast: Toast | null): Promise<void> {
  if (toast === null) return;
  let granted = await isPermissionGranted();
  if (!granted) {
    granted = (await requestPermission()) === "granted";
  }
  if (!granted) return;
  sendNotification({ title: toast.title, body: toast.body });
}

/**
 * The job event carries only the outcome, so the row it refers to is read back
 * from the ledger. A failure is looked up in the failed-only page; a success is
 * the newest `done` row. Taking whichever row happens to be newest would name
 * the wrong file as soon as another job was enqueued in between.
 */
async function jobFor(outcome: JobOutcome): Promise<PrintJob | null> {
  if (outcome === "Failed") {
    const rows = await api.listJobs(true, 1);
    return rows[0] ?? null;
  }
  const rows = await api.listJobs(false, 20);
  return rows.find((j) => j.state === "done") ?? null;
}

export function useJobNotifications(): void {
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });

  // The listeners are registered once; the mode is read through a ref so a
  // settings change takes effect without tearing down the subscription.
  const modeRef = useRef<NotificationMode>("all");
  modeRef.current = (settings.data?.notification_mode ?? "all") as NotificationMode;

  useEffect(() => {
    const jobSub = onJobEvent((e) => {
      if (!shouldNotify(modeRef.current, e.outcome)) return;
      void jobFor(e.outcome)
        .then((job) => deliver(jobNotification(e.outcome, job)))
        .catch(() => {
          // A toast is never worth breaking the app over.
        });
    });

    const queueSub = onQueueEvent((e) => {
      // A stalled printer is an error condition, so it passes the errors mode.
      if (modeRef.current === "off") return;
      void deliver(queueNotification(e)).catch(() => {});
    });

    return () => {
      void jobSub.then((un) => un());
      void queueSub.then((un) => un());
    };
  }, []);
}
```

- [ ] **Step 5: Call the hook from the shell**

In `src/App.tsx`, add the import next to the existing ones:

```tsx
import { useJobNotifications } from "./lib/useJobNotifications";
```

and call it as the first statement inside `function App()`, before the `return`:

```tsx
function App(): JSX.Element {
  useJobNotifications();

  return (
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npx vitest run src/lib/notify.test.ts`
Expected: PASS — 10 tests.

Run: `npm test`
Expected: PASS — every suite in the project.

Run: `npx tsc --noEmit`
Expected: no type errors.

Run: `npm run build`
Expected: the production bundle builds.

- [ ] **Step 7: Commit**

```bash
git add src/lib/notify.ts src/lib/notify.test.ts src/lib/useJobNotifications.ts src/App.tsx
git commit -m "feat: add desktop notifications gated by the notification mode setting"
```

---

## Manual verification

Automated tests cannot see a tray icon or a Windows toast. Run this list once the
twelve tasks are green, and add it to `docs/WINDOWS-VERIFICATION.md` created by
plan 1's Task 17:

1. The sidebar shows the Printy lockup in cream on the teal gradient; the folder
   mark is legible against it.
2. Ordner is the start screen; its active nav item is coral, not mint.
3. The aggregate line matches the folder list and the day's prints.
4. The global pause switch greys every folder dot and turns the tray icon grey.
5. A printer without duplex shows the duplex control **greyed out and labelled**,
   never hidden.
6. Creating a folder with files already in it and the checkbox left alone prints
   nothing; ticking the checkbox prints them.
7. Switching the printer off turns the tray icon coral and shows
   "Wartet auf Drucker" on every enabled folder; switching it on restores mint.
8. Closing the window keeps the tray icon and keeps printing; Beenden in the
   tray menu stops the app.
9. With autostart and "Minimiert starten" on, a reboot brings Printy back in the
   tray with no window flash.
10. Notification mode "Nur Fehler" stays silent on a successful print and toasts
    on a failure; "Aus" stays silent for both.
11. Switching to the dark theme repaints every screen, and the sidebar and logo
    stay legible.

## What this plan does not cover

The engine itself — watcher, intake, queue, print backends, database and the
fourteen commands of spec section 11 — is plan 1's. This plan adds only three
Rust commands (`count_existing_files_cmd`, `get_autostart_cmd`,
`set_autostart_cmd`) and the `shell` module; it changes no existing backend
behaviour.
