import { create } from "zustand";

export type Theme = "light" | "dark" | "system";
type Resolved = "light" | "dark";

const STORAGE_KEY = "synapse-theme";

function systemPrefersDark(): boolean {
  return typeof window !== "undefined" && window.matchMedia
    ? window.matchMedia("(prefers-color-scheme: dark)").matches
    : false;
}

function resolveTheme(theme: Theme): Resolved {
  return theme === "system" ? (systemPrefersDark() ? "dark" : "light") : theme;
}

/** Apply the resolved theme to <html> and return it. */
function applyTheme(theme: Theme): Resolved {
  const resolved = resolveTheme(theme);
  if (typeof document !== "undefined") {
    document.documentElement.classList.toggle("dark", resolved === "dark");
  }
  return resolved;
}

function readStored(): Theme {
  if (typeof localStorage === "undefined") return "system";
  const value = localStorage.getItem(STORAGE_KEY);
  return value === "light" || value === "dark" || value === "system" ? value : "system";
}

interface ThemeState {
  theme: Theme;
  resolved: Resolved;
  setTheme: (theme: Theme) => void;
}

const initial = readStored();

export const useTheme = create<ThemeState>((set) => ({
  theme: initial,
  resolved: applyTheme(initial),
  setTheme: (theme) => {
    if (typeof localStorage !== "undefined") localStorage.setItem(STORAGE_KEY, theme);
    set({ theme, resolved: applyTheme(theme) });
  },
}));
