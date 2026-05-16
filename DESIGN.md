---
name: UE Cache Manager
description: Cross-machine Unreal Engine cache management for VP/XR render clusters.
colors:
  bg: "oklch(0.985 0.002 240)"
  bg-dark: "oklch(0.16 0.005 240)"
  fg: "oklch(0.22 0.01 240)"
  fg-dark: "oklch(0.96 0 0)"
  surface-card: "oklch(1 0 0)"
  surface-card-dark: "oklch(0.195 0.005 240)"
  surface-muted: "oklch(0.955 0.005 240)"
  surface-muted-dark: "oklch(0.235 0.005 240)"
  sidebar: "oklch(0.97 0.005 240)"
  sidebar-dark: "oklch(0.175 0.005 240)"
  border: "oklch(0.9 0.005 240)"
  border-dark: "oklch(0.27 0.005 240)"
  muted-foreground: "oklch(0.5 0.01 240)"
  muted-foreground-dark: "oklch(0.68 0.01 240)"
  primary: "oklch(0.58 0.13 210)"
  primary-dark: "oklch(0.78 0.13 200)"
  destructive: "oklch(0.55 0.22 25)"
  destructive-dark: "oklch(0.62 0.22 25)"
  status-healthy: "oklch(0.55 0.15 152)"
  status-healthy-dark: "oklch(0.72 0.16 152)"
  status-warning: "oklch(0.65 0.16 75)"
  status-warning-dark: "oklch(0.78 0.16 75)"
  status-critical: "oklch(0.55 0.22 25)"
  status-critical-dark: "oklch(0.65 0.22 25)"
  status-info: "oklch(0.58 0.13 210)"
  status-offline: "oklch(0.65 0.005 240)"
  status-unknown: "oklch(0.7 0.008 240)"
typography:
  display:
    fontFamily: "Manrope, -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 800
    lineHeight: 1.1
    letterSpacing: "normal"
    fontFeature: "'cv11', 'ss01', 'ss03'"
  title:
    fontFamily: "Manrope, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 800
    lineHeight: 1.3
    letterSpacing: "normal"
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, 'Helvetica Neue', Arial, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.55
    letterSpacing: "normal"
  label:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace"
    fontSize: "0.6875rem"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "0.04em"
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.5
rounded:
  sm: "4px"
  md: "6px"
  lg: "8px"
  xl: "12px"
  pill: "9999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
  "2xl": "32px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.bg}"
    typography: "{typography.title}"
    rounded: "{rounded.md}"
    padding: "0 16px"
    height: "36px"
  button-primary-hover:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.bg}"
  button-outline:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    padding: "0 16px"
    height: "36px"
  button-ghost:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    height: "36px"
  button-destructive:
    backgroundColor: "{colors.destructive}"
    textColor: "{colors.bg}"
    rounded: "{rounded.md}"
    padding: "0 16px"
    height: "36px"
  input:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    padding: "4px 12px"
    height: "36px"
  card:
    backgroundColor: "{colors.surface-card}"
    textColor: "{colors.fg}"
    rounded: "{rounded.lg}"
    padding: "16px"
  status-badge:
    backgroundColor: "{colors.surface-muted}"
    textColor: "{colors.fg}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: "0 10px"
    height: "28px"
  status-dot:
    backgroundColor: "{colors.status-healthy}"
    rounded: "{rounded.pill}"
    width: "10px"
    height: "10px"
  kpi-tile:
    backgroundColor: "{colors.surface-card}"
    textColor: "{colors.fg}"
    padding: "16px"
  sidebar-nav-item:
    backgroundColor: "{colors.sidebar}"
    textColor: "{colors.muted-foreground}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: "0 12px"
    height: "40px"
---

# Design System: UE Cache Manager

## 1. Overview

**Creative North Star: "The Operator's Console"**

UECM is a workstation tool for VP/XR technical directors who spend their day cross-checking a small fleet of Windows render nodes. The interface borrows the discipline of macOS System Settings (clear grouping, generous whitespace, native chrome) and the typographic clarity of Vercel / Linear (one strong display face, calm neutrals, status colors used sparingly). It is **not** a NOC control room, an IT admin console from 2014, or a consumer gaming utility.

The system is built on OKLCH tokens, light + dark surfaces, a Manrope display face paired with system-sans body type, and a six-tone status palette. Density sits between Linear and macOS: closer to Linear when reading cluster tables, closer to macOS when reading a single machine detail. The visual mood is **calm, trustworthy, production-safe**: every destructive operation must feel deliberate, never accidental. Color is information, not decoration.

The system rejects: dashboard theater (ten gradient KPI tiles competing for attention), neon-on-black observability vocabulary, gaming-grade orange CTAs, and SaaS-cream hero cards with emoji icons. UECM looks like a tool, not a pitch.

**Key Characteristics:**

- OKLCH-only color tokens, light + dark themes from the same hue family
- Manrope variable display + system-sans body + monospace labels
- Status color is rare, intentional, always paired with an icon or text
- Flat by default: depth comes from token-tinted surfaces, not shadow
- Components extend reka-ui + class-variance-authority primitives

## 2. Colors

A near-neutral interface tinted slightly cool, with one steel-blue primary, three semantic status colors (green / amber / red), and a calm cyan accent for `info`. Color is used to **classify**, never to decorate.

### Primary

- **Steel Blue** (`oklch(0.58 0.13 210)` light / `oklch(0.78 0.13 200)` dark): The single brand accent. Appears on primary buttons, focused inputs, sidebar nav active state, link text, and the `info` semantic tone. Dark mode shifts to a brighter, slightly more cyan hue so it reads against the dark navy surface without glowing.

### Status (semantic, not decorative)

- **Healthy Green** (`oklch(0.55 0.15 152)` / `oklch(0.72 0.16 152)`): Online machines, passing checks, applied configurations.
- **Warning Amber** (`oklch(0.65 0.16 75)` / `oklch(0.78 0.16 75)`): Drift, partial reachability, GPU/driver mismatch, INI advisories.
- **Critical Red** (`oklch(0.55 0.22 25)` / `oklch(0.65 0.22 25)`): Offline machines, failed operations, broken DDC paths, destructive confirmations. Reused as `destructive`.
- **Offline / Unknown** (`oklch(0.65 0.005 240)` / `oklch(0.7 0.008 240)`): Near-neutral greys for "haven't seen this machine yet" and "probe in flight".

### Neutral (cool-tinted, never `#000` / `#fff`)

- **Background** (`oklch(0.985 0.002 240)` light / `oklch(0.16 0.005 240)` dark): The base surface. Light is a paper-white tinted toward the brand hue; dark is a slate navy.
- **Card / Popover** (`oklch(1 0 0)` / `oklch(0.195 0.005 240)`): Raised content surfaces. Light is pure white only because the background is already tinted; dark stays one tone above background.
- **Muted** (`oklch(0.955 0.005 240)` / `oklch(0.235 0.005 240)`): Internal surfaces (table row hover, badge backgrounds at 10% alpha, secondary buttons).
- **Sidebar** (`oklch(0.97 0.005 240)` / `oklch(0.175 0.005 240)`): The persistent left rail. Sits between Background and Card; gives the rail a felt boundary without a hard divider.
- **Border** (`oklch(0.9 0.005 240)` / `oklch(0.27 0.005 240)`): Hairlines, table dividers, input strokes. Never `#000`/`#fff`; always tinted toward the cool hue family.
- **Foreground** (`oklch(0.22 0.01 240)` / `oklch(0.96 0 0)`): Primary text.
- **Muted Foreground** (`oklch(0.5 0.01 240)` / `oklch(0.68 0.01 240)`): Secondary text, labels, placeholders.

### Named Rules

**The Status-as-Information Rule.** Status color exists to answer one question: is this thing okay? It is paired with an icon (`check-circle-2`, `alert-triangle`, etc.) AND text every time it appears. A user who cannot see color must still get the answer.

**The 10% Accent Rule.** The Steel Blue primary covers no more than 10% of any given screen. It earns its weight by being rare. If a layout looks "too grey", the answer is hierarchy through scale and weight, not more color.

**The No-Stoplight Rule.** Healthy / Warning / Critical never appear in the same row as decorative color. A row is either neutral with one status badge, or it is silent. UECM is not a stoplight panel.

## 3. Typography

**Display Font:** Manrope (variable woff2, weight 100–900), with system-sans fallback
**Body Font:** -apple-system / BlinkMacSystemFont / Segoe UI / system-ui
**Label / Mono Font:** ui-monospace / SF Mono / Menlo / Consolas

**Character:** Manrope's geometric humanism gives the display layer a calm authority, closer to Inter / Geist than to a magazine serif. The body layer hands off to the host OS sans, so on Windows it reads as a native tool and on macOS as a native app. Monospace is reserved for *labels and machine output*, not body copy.

Font features `'cv11', 'ss01', 'ss03'` are enabled globally so Manrope and the system stacks render with consistent open-aperture digits and disambiguated alternates.

### Hierarchy

- **Display** (weight 800, `1.5rem` / 24px, line-height 1.1, `font-display`): KPI tile values, page H1, sidebar app name, cluster score. Always Manrope, always bold.
- **Title** (weight 700–800, `0.875rem` / 14px, line-height 1.3): Card headings, section titles, table column headers. Manrope.
- **Body** (weight 400, `0.875rem` / 14px, line-height 1.55): All paragraph and form text. System sans.
- **Label** (weight 700, `0.6875rem` / 11px, letter-spacing `0.04em`, uppercase, mono): KPI tile labels, badge text, table micro-headers. The monospace + uppercase combination signals "this is a classification, not prose".
- **Mono** (weight 400, `0.8125rem` / 13px, line-height 1.5): Machine names, IP addresses, registry paths, INI diff blocks, command output. Anything the user might copy verbatim into a terminal.

### Named Rules

**The Mono-for-Identifiers Rule.** Any string a TD might paste into PowerShell (hostnames, IPs, paths, env var names, registry keys, GPU model strings) renders in mono. Prose renders in sans. Mixing them inside one sentence is allowed and encouraged: *"Registry value `qwMemorySize` on `lanpc` was 10240."*

**The CJK Comfort Rule.** When Chinese is active, do not apply negative letter-spacing, and bump body line-height by one step (1.55 → 1.65). Mono fonts have no CJK glyphs and will fall back per-character; never wrap Chinese inside `.font-mono`.

**The Scale Ratio Rule.** Display is ~1.7× Title is ~1× Body is ~1× Label is ~0.79×. The display step alone carries the contrast; intermediate sizes are forbidden: no `text-base` / `text-lg` body, no `text-xl` headings. The system has four type sizes, not eight.

## 4. Elevation

UECM is **flat by default**. Depth is conveyed by token-tinted surfaces, not by shadow. The Sidebar → Background → Card progression in OKLCH lightness creates a three-tier hierarchy without any drop shadow.

Shadow appears in exactly two places, both functional:

### Shadow Vocabulary

- **input** (`box-shadow: 0 1px 2px 0 rgba(0,0,0,0.05)`, applied via Tailwind `shadow-sm`): Inputs only. A 1px inset hint that an editable field exists, used to compensate for the transparent background of native form controls.
- **focus-ring** (`box-shadow: 0 0 0 2px oklch(var(--ring) / 0.5)`, applied via `focus-visible:ring-2 focus-visible:ring-ring`): Every interactive element. The only "elevation" most components ever get is on focus, and that elevation is the brand color, not a grey shadow.

### Named Rules

**The Flat-By-Default Rule.** Surfaces are flat at rest. If you find yourself reaching for `box-shadow` to separate two surfaces, choose a different surface token instead (`bg-card` over `bg-background`, `bg-sidebar` over `bg-card`).

**The No-Glass Rule.** No `backdrop-filter: blur`. No glassmorphism. No semi-transparent navigation. Surfaces are opaque and stable; the focus is the data on them, not the surface itself.

## 5. Components

Every component below extends [reka-ui](https://reka-ui.com/) primitives and is styled through `class-variance-authority` variant maps. Tokens come from `tailwind.config.js`, which wraps the bare OKLCH components in `oklch(var(--token) / <alpha-value>)` so Tailwind alpha modifiers (`bg-primary/90`, `bg-status-healthy/10`) work everywhere.

### Buttons

- **Shape:** Gently rounded (`rounded-md`, 6px). Never pill-shaped, never square.
- **Heights:** `sm` 36px (px-3), `default` 36px (px-4), `lg` 44px (px-6), `icon` 36×36. No 32px / 40px in-between.
- **Typography:** `font-display` (Manrope), weight 700, `text-sm` (14px). Never body-font on buttons.
- **Primary:** `bg-primary text-primary-foreground hover:bg-primary/90`. Steel Blue ground, white-tinted text.
- **Outline:** `border border-input bg-background hover:bg-accent`. The default for table row actions.
- **Secondary:** `bg-secondary text-secondary-foreground hover:bg-secondary/80`. Muted neutral; for in-card actions where Outline would compete with row borders.
- **Ghost:** `hover:bg-accent`. For dense toolbars where button chrome would overwhelm.
- **Destructive:** `bg-destructive text-destructive-foreground hover:bg-destructive/90`. Critical Red. Reserved for actions that change cluster state irreversibly. Never the *default* button in a confirmation dialog; the destructive variant goes on the right, the default action is `outline`.
- **Focus:** `focus-visible:ring-2 focus-visible:ring-ring`. Steel Blue ring, 2px, only on keyboard focus.

### Inputs

- **Shape:** `rounded-md` (6px), `h-9` (36px), `px-3 py-1`.
- **Background:** Transparent on top of the parent surface, so an input inside a Card looks like a Card-toned input and an input inside the sidebar looks like a sidebar-toned input.
- **Border:** `border-input` (1px hairline).
- **Shadow:** `shadow-sm` (see Elevation).
- **Placeholder:** `text-muted-foreground`.
- **Focus:** Border shifts to `border-ring`, plus a 2px Steel Blue ring at 50% alpha. Never a glow.

### Status Badge

The signature classification chip. Pill-shaped (`rounded-full`), 28px tall, 10px horizontal padding. Layout is `icon + label` with `gap-1.5`. Background is the status color at 10% alpha; border is the same status color at 30% alpha; text is the full status color. Label is `font-display font-bold uppercase tracking-wide`: *typography reinforces classification*. Every badge ships with an icon (`check-circle-2` for healthy, `alert-triangle` for critical, `info` for info / unknown). **A status badge without an icon is forbidden.**

Tones: `healthy` / `warning` / `critical` / `info` / `offline` / `unknown` / `progress` / `na`. `offline` and `unknown` collapse their text to `muted-foreground` so they recede; the badge is present but quiet.

### Status Dot

The minimalist counterpart to Status Badge. 10×10px solid disc. Used in dense tables, sidebar headers, and anywhere a full badge would steal too much space. Optional `pulse` prop layers an animated `ping` for "probe in flight" (the only animation in the system that draws active attention).

### KPI Tile

A flat block (`bg-card`, `p-4`, `min-h-20`) with two lines: an uppercase mono label (`text-[11px] font-bold uppercase`, muted-foreground) over a display value (`font-display text-2xl font-extrabold`). Tone-bound: the value color follows the status tone (`text-status-healthy`, `text-status-critical`, etc.). **There is exactly one tone per tile**; if a metric needs to show three states at once, it is not a KPI Tile, it is a list.

### Cards

- **Corner:** `rounded-lg` (8px) for top-level cards; `rounded-md` (6px) for nested content blocks.
- **Background:** `bg-card` (pure white in light, one tone above background in dark).
- **Border:** 1px `border-border`; no shadow.
- **Padding:** 16px (`p-4`) default, 24px (`p-6`) for full-page detail panels.
- **Nested cards are forbidden.** If you need to group inside a card, use a `bg-muted` block with no border, or a horizontal divider with a section label.

### Sidebar Navigation

240px wide (`w-60`), full-height, `border-r` against the main canvas. Header: 64px tall, app mark + name + cluster summary. Nav items: 40px tall, `rounded-md`, `gap-3` between icon and label. Active state is `bg-sidebar-accent text-sidebar-accent-foreground`: a small tonal lift, never the brand primary as a background. Footer panel ships the cluster score: a Manrope 2xl value, a status dot, and a critical / warning summary line.

### Code Block (`UecmCodeBlock`)

For showing INI diffs, env-var before/after, PowerShell output. Always mono, 13px, `bg-muted`, `rounded-md`, generous left padding. Diff colors use the status palette: `+` lines tinted with `status-healthy` at 10% alpha, `-` lines tinted with `status-critical` at 10%. Never use a separate diff palette; the cluster's status colors carry through.

### Filter Chip (`UecmFilterChip`)

Pill-shaped toggle. Resting state is `bg-muted text-muted-foreground`; selected state lifts to `bg-accent text-foreground` with a 1px border in `border-ring`. Filters are *additive*, never modal: clicking a chip narrows the visible set, never opens a dialog.

## 6. Do's and Don'ts

### Do

- **Do** keep one Steel Blue accent per screen. Hierarchy comes from size and weight, not color saturation.
- **Do** pair every status color with an icon and a text label. Color is the third channel, not the first.
- **Do** use `font-mono` for any string a TD might paste into PowerShell: hostnames, IPs, paths, env var names, registry keys.
- **Do** put backup paths and impact summaries on screen *before* the destructive button. Operations carry weight (Principle 2 in PRODUCT.md).
- **Do** use OKLCH tokens via `oklch(var(--name) / <alpha-value>)`. Never hand-write `#hex` or `rgb()` in components.
- **Do** test every new surface in both light and dark before committing.
- **Do** scale typography in four steps only: Display / Title / Body / Label.

### Don't

- **Don't** build a "dense IT table" with zebra stripes, 11px text, and a tooltip on every cell. UECM is not Windows Server Manager.
- **Don't** build a "NOC dashboard" with five tones of green and pulsing borders. UECM is not Grafana.
- **Don't** use gaming-grade orange CTAs, gradient buttons, or large radii (>12px). UECM is not Razer Synapse.
- **Don't** wrap empty screens in soft-gradient hero cards with three KPI tiles and an emoji. UECM is not a SaaS landing page.
- **Don't** use `border-left` greater than 1px as a colored stripe on cards or list items. Forbidden.
- **Don't** put gradient text or `background-clip: text` anywhere.
- **Don't** add `backdrop-filter: blur` for "glass" navigation. Surfaces are opaque.
- **Don't** reach for a modal as the first thought. Inline confirmation, drawer, or in-place editor first.
- **Don't** use `#000` or `#fff`. Every neutral tints toward the cool hue family.
- **Don't** nest cards. If you need a sub-group, use `bg-muted` or a divider with a label.
- **Don't** stack two status colors in the same row. One classification per item.
- **Don't** animate layout properties (`width`, `height`, `top`, `left`). Animate `opacity` and `transform` only.
- **Don't** use em dashes (`—`) or `--` in copy. Commas, colons, semicolons, periods, parentheses.
