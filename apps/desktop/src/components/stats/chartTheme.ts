/** Shared recharts styling so every panel reads as one system. */

export const categorical = [
  "hsl(var(--chart-1))",
  "hsl(var(--chart-2))",
  "hsl(var(--chart-3))",
  "hsl(var(--chart-4))",
  "hsl(var(--chart-5))",
  "hsl(var(--chart-6))",
] as const;

export const sequential = [
  "hsl(var(--seq-1))",
  "hsl(var(--seq-2))",
  "hsl(var(--seq-3))",
  "hsl(var(--seq-4))",
  "hsl(var(--seq-5))",
] as const;

export const axisProps = {
  tick: { fontSize: 11, fill: "hsl(var(--muted-foreground))" },
  stroke: "hsl(var(--border))",
  tickLine: false,
} as const;

export const tooltipStyle = {
  background: "hsl(var(--popover))",
  border: "1px solid hsl(var(--border))",
  borderRadius: 8,
  fontSize: 12,
  color: "hsl(var(--popover-foreground))",
  boxShadow: "0 4px 16px rgba(0,0,0,0.12)",
} as const;

/**
 * Recharts injects each tooltip row's own series color as text color by
 * default, which reads fine in light mode but can go dark-on-dark (or just
 * low-contrast) once the popover surface flips dark. Force readable ink.
 */
export const tooltipItemStyle = { color: "hsl(var(--popover-foreground))" } as const;
export const tooltipLabelStyle = { color: "hsl(var(--popover-foreground))" } as const;

export const gridProps = {
  stroke: "hsl(var(--border))",
  strokeDasharray: "3 3",
  vertical: false,
} as const;
