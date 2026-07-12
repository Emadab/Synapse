---
name: Synapse
description: An Anki-compatible spaced-repetition study app, rebuilt as a precise, calm, fast desktop instrument.
colors:
  primary: "#4A5FD1"
  primary-dark: "#7089DB"
  background: "#FCFCFD"
  background-dark: "#0D1017"
  foreground: "#171B26"
  foreground-dark: "#E7E9EE"
  border: "#EBECF0"
  muted-foreground: "#6B7280"
  again: "#EF4444"
  hard: "#F59E0B"
  good: "#22C55E"
  easy: "#3B82F6"
  chart-1-blue: "#3182CE"
  chart-2-aqua: "#0D9F6E"
  chart-3-yellow: "#EBA100"
  chart-4-green: "#008500"
  chart-5-violet: "#4E43AD"
  chart-6-red: "#E14040"
typography:
  body:
    fontFamily: "Inter var, Inter, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "1.5"
    letterSpacing: "normal"
  headline:
    fontFamily: "Inter var, Inter, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "clamp(1.25rem, 2vw, 1.75rem)"
    fontWeight: 600
    lineHeight: "1.2"
    letterSpacing: "-0.02em"
  label:
    fontFamily: "Inter var, Inter, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: "1.3"
    letterSpacing: "normal"
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "1.4"
rounded:
  sm: "4px"
  md: "8px"
  lg: "12px"
  xl: "16px"
spacing:
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "32px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "#FFFFFF"
    rounded: "{rounded.sm}"
    padding: "8px 16px"
  button-primary-hover:
    backgroundColor: "#3E51B8"
  button-secondary:
    backgroundColor: "#F1F2F6"
    textColor: "{colors.foreground}"
    rounded: "{rounded.sm}"
    padding: "8px 16px"
  card:
    backgroundColor: "#FFFFFF"
    textColor: "{colors.foreground}"
    rounded: "{rounded.lg}"
    padding: "16px"
  input:
    backgroundColor: "#FFFFFF"
    textColor: "{colors.foreground}"
    rounded: "{rounded.sm}"
    padding: "8px 12px"
---

# Design System: Synapse

## 1. Overview

**Creative North Star: "The Instrument Panel"**

Synapse looks like a well-calibrated instrument, not a decorated app. Every surface exists because it does a job: chrome tells you where you are, glass separates what's persistent from what's scrolling past, color marks state and nothing else. The palette is a single indigo signal against cool, near-neutral grays — precise, calm, fast, the same three words that describe the study experience it wraps.

This system explicitly rejects Anki's cluttered, inconsistent, dated interface and rejects gamified study-app conventions in equal measure — no mascots, no cartoonish celebration, no forced streak confetti. Confidence here is earned through consistency and restraint, not decoration.

**Key Characteristics:**

- Single-hue indigo brand signal (#4A5FD1 light / #7089DB dark), used sparingly and consistently for interactive/primary state
- Cool, low-chroma neutral shell (blue-gray, not warm) in both themes
- Structural glass: blur + saturate reserved for chrome that separates persistent UI from scrollable content, never applied decoratively
- Fixed-order semantic colors for review state (again/hard/good/easy) that never shift meaning
- Tight, confident geometry: small radii (4–16px), crisp borders, immediate feedback on hover/focus

## 2. Colors

A single indigo signal against a cool-gray instrument shell; color is functional, not atmospheric.

### Primary

- **Instrument Indigo** (`#4A5FD1` light / `#7089DB` dark): the one interactive/brand color — primary buttons, active nav state, focus rings, links. Consistent across light and dark, lightened in dark mode for legibility rather than recolored.

### Neutral

- **Panel White / Panel Ink** (`#FCFCFD` background light / `#0D1017` background dark): the base shell. Cool, almost-imperceptible blue tint, never warm.
- **Instrument Text** (`#171B26` foreground light / `#E7E9EE` foreground dark): body and heading text, tuned for AA contrast against the panel background.
- **Hairline Border** (`#EBECF0` light / equivalent low-chroma slate in dark): dividers, card edges, input strokes — always subtle, never a design statement.
- **Instrument Gray** (`#6B7280`): muted/secondary text — timestamps, helper copy, disabled labels.

### Semantic (review state — fixed, never reassigned)

- **Again** (`#EF4444`): failed recall.
- **Hard** (`#F59E0B`): recalled with difficulty.
- **Good** (`#22C55E`): recalled correctly.
- **Easy** (`#3B82F6`): recalled with no effort.

### Named Rules

**The One Signal Rule.** Indigo is the only brand color and it means one thing: interactive or primary. It is never used decoratively (no gradients, no indigo-tinted backgrounds for atmosphere).

**The Fixed Palette Rule.** Chart series and answer-button colors (again/hard/good/easy, chart-1 through chart-6) are assigned by role, not by index or filter state. A color always means the same state across every screen.

## 3. Typography

**Body Font:** Inter var, with Inter, ui-sans-serif, system-ui, -apple-system, Segoe UI as fallbacks
**Label/Mono Font:** ui-monospace, SFMono-Regular, Menlo (used for numeric review stats — tabular-nums)

**Character:** A single, precise system-adjacent sans across the entire app — no display serif, no decorative pairing. The typeface itself carries the "instrument" feel: neutral, legible, fast to scan.

### Hierarchy

- **Headline** (600, `clamp(1.25rem, 2vw, 1.75rem)`, 1.2 line-height, -0.02em tracking): screen titles, deck names, section headers. Tight tracking keeps headlines dense rather than airy.
- **Body** (400, 14px, 1.5 line-height): default UI text, card content, list rows.
- **Label** (500, 12px, 1.3 line-height): buttons, badges, form labels, chart legends.
- **Mono/Numeric** (400, 13px, tabular-nums): review counts, stats figures, timers — anything where digits must align in a column.

### Named Rules

**The One Family Rule.** Inter carries every role via weight and size, never a second typeface. A display serif or script face would contradict the instrument-panel premise.

## 4. Elevation

Synapse is flat at rest. Depth is not conveyed with drop shadows on cards or buttons — it's conveyed structurally, through translucent "glass" chrome that separates persistent UI (titlebar, sidebar, sticky headers, floating panels) from the content scrolling beneath it. Glass is functional layering, not a decorative effect; it exists only where content needs to visibly pass underneath fixed UI.

### Shadow Vocabulary

- **Glass Chrome** (`backdrop-filter: blur(16px) saturate(1.4)` over `hsl(var(--glass-chrome-bg))`, ~72% opacity): titlebar, sidebar, sticky screen headers.
- **Glass Panel** (`backdrop-filter: blur(24px) saturate(1.5)` over `hsl(var(--glass-panel-bg))`, ~82% opacity): command palette, dialogs, popovers, the study HUD — deeper blur than chrome since these sit further above content.
- Both fall back to a fully solid surface color when `backdrop-filter` isn't supported — never a broken transparent panel.

### Named Rules

**The Structural Glass Rule.** Blur and saturation are reserved for chrome that must stay legible over arbitrary scrolled content. Cards, buttons, and static panels stay flat; glass is never added purely for atmosphere.

**The Vignette Exception.** Focus mode's radial vignette (`radial-gradient(ellipse at center, transparent 45%, hsl(var(--background)) 100%)` in `StudySessionScreen.tsx`) is the one deliberate gradient in the system. It exists to pull attention toward the card when chrome is hidden — a functional focus cue, not decoration — and stays a single neutral hue (the background color fading to itself) rather than a color or brand gradient. This is the only sanctioned exception to the no-gradient rule below; it does not open the door to gradients elsewhere.

## 5. Components

Buttons, inputs, and cards read as tight and confident: small radii, crisp hairline borders, and feedback that lands immediately on hover and focus — nothing loose, bouncy, or delayed.

### Buttons

- **Shape:** small radius (`4px`, `rounded.sm`), never pill-shaped.
- **Primary:** Instrument Indigo background, white text, `8px 16px` padding.
- **Hover / Focus:** background steps one shade darker (`#3E51B8`) on hover; `2px solid` themed ring at `2px` offset on focus-visible, replacing the browser default outline everywhere.
- **Secondary / Ghost:** neutral background (`#F1F2F6` light, dark-mode equivalent secondary token), foreground text, same radius and padding as primary — differs by fill, not by shape.

### Cards / Containers

- **Corner Style:** `12px` radius (`rounded.lg`).
- **Background:** solid card surface color (`#FFFFFF` light / dark card token) — no gradients, no glass.
- **Shadow Strategy:** none at rest; see Elevation. Cards are flat.
- **Border:** `1px` hairline border in the border token where separation is needed; otherwise background contrast alone.
- **Internal Padding:** `16px` (`spacing.md`).

### Inputs / Fields

- **Style:** `1px` hairline border, solid background, `4px` radius, `8px 12px` padding.
- **Focus:** border shifts to the ring color, `2px` focus-visible outline, no glow or shadow.
- **Error / Disabled:** destructive token for error state (`hsl(var(--destructive))`); disabled drops to muted foreground with reduced opacity, never a separate gray-out overlay.

### Navigation

- Sidebar and titlebar use Glass Chrome (see Elevation), sit persistently, and use Instrument Indigo for the active-item state only — inactive items stay in muted foreground until hovered or selected.

### Review Answer Buttons (signature component)

The four-button row (Again / Hard / Good / Easy) is the app's most-repeated interaction. Each button carries its fixed semantic color as a top-level fill, not a subtle tint, so the choice is legible at a glance during fast review — the one place in the system where color is allowed to be loud, because speed and clarity here matter more than restraint.

## 6. Do's and Don'ts

### Do:

- **Do** use Instrument Indigo (`#4A5FD1` / `#7089DB`) as the only brand/interactive color, consistently across light and dark.
- **Do** keep glass (`blur` + `saturate`) structural — chrome only, never decorative panels or cards.
- **Do** keep radii small (`4–16px`) and borders crisp — the "tight and confident" component feel.
- **Do** honor `prefers-reduced-motion` on every transition; motion durations stay fast (120–280ms).
- **Do** keep chart and review-state colors fixed by role (again/hard/good/easy, chart-1..6) — never reassign by index or filter.

### Don't:

- **Don't** replicate Anki's cluttered, inconsistent, dated interface — no dense unstyled toolbars, no inconsistent spacing, no mismatched dialog chrome.
- **Don't** add gamification: no mascots, no cartoonish confetti, no forced streak celebrations. Synapse's confidence is quiet, not performative.
- **Don't** use `border-left`/`border-right` as a colored accent stripe on cards or list rows.
- **Don't** add drop shadows to cards or buttons at rest — depth comes from structural glass only, not decorative elevation.
- **Don't** introduce a second typeface. Inter carries every role via weight and size.
- **Don't** use gradient text or `background-clip: text` for emphasis — weight and size carry hierarchy instead.
