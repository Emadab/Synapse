// Synapse design tokens.
//
// The runtime theme is driven by CSS variables in the app's globals.css (the
// shadcn convention), but these tokens are the canonical reference for values
// that live outside CSS — e.g. chart series colors (Recharts), canvas drawing,
// or a future native mobile UI that cannot read the web CSS variables.

/** Brand accent ramp (indigo/violet family — the Linear/Arc-adjacent feel). */
export const brand = {
  50: "#eef2ff",
  100: "#e0e7ff",
  200: "#c7d2fe",
  300: "#a5b4fc",
  400: "#818cf8",
  500: "#6366f1",
  600: "#4f46e5",
  700: "#4338ca",
  800: "#3730a3",
  900: "#312e81",
} as const;

/** Semantic feedback colors used for answer buttons, toasts and badges. */
export const semantic = {
  again: "#ef4444", // red-500
  hard: "#f59e0b", // amber-500
  good: "#22c55e", // green-500
  easy: "#3b82f6", // blue-500
  info: "#6366f1",
} as const;

/** Border radius scale (px). Mirrors the `--radius` CSS variable. */
export const radii = {
  sm: 6,
  md: 8,
  lg: 12,
  xl: 16,
} as const;

/** Motion: durations (ms) and easing curves. Animations must never block input. */
export const motion = {
  duration: { fast: 120, base: 180, slow: 280 },
  easing: {
    standard: [0.2, 0, 0, 1] as const,
    emphasized: [0.3, 0, 0, 1] as const,
  },
} as const;

/** Ordered palette for chart series (heatmaps, retention, forecast). */
export const chartSeries = [
  brand[500],
  semantic.good,
  semantic.easy,
  semantic.hard,
  semantic.again,
  brand[300],
] as const;
