import { create } from "zustand";

export type HomeLayout = "list" | "grid" | "hero";

const HOME_LAYOUT_KEY = "synapse-home-layout";

function readHomeLayout(): HomeLayout {
  if (typeof localStorage === "undefined") return "list";
  const value = localStorage.getItem(HOME_LAYOUT_KEY);
  return value === "list" || value === "grid" || value === "hero" ? value : "list";
}

interface UiState {
  /** Study focus mode — hides sidebar/chrome. Intentionally not persisted. */
  focusMode: boolean;
  setFocusMode: (value: boolean) => void;
  toggleFocusMode: () => void;

  /** Lets chrome outside CommandPalette (e.g. TitleBar) request it open. */
  paletteOpenSignal: number;
  openPalette: () => void;

  homeLayout: HomeLayout;
  setHomeLayout: (layout: HomeLayout) => void;
}

export const useUi = create<UiState>((set) => ({
  focusMode: false,
  setFocusMode: (value) => set({ focusMode: value }),
  toggleFocusMode: () => set((s) => ({ focusMode: !s.focusMode })),

  paletteOpenSignal: 0,
  openPalette: () => set((s) => ({ paletteOpenSignal: s.paletteOpenSignal + 1 })),

  homeLayout: readHomeLayout(),
  setHomeLayout: (layout) => {
    if (typeof localStorage !== "undefined") localStorage.setItem(HOME_LAYOUT_KEY, layout);
    set({ homeLayout: layout });
  },
}));
