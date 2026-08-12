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
