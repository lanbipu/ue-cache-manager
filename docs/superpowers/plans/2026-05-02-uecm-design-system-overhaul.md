# UECM Design System Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current ad-hoc Tailwind styling with a complete dark/light design system based on the Next.js + shadcn/ui mockup at `/Users/bip.lan/Downloads/b_sVGHApfkUd9`, preserving all existing functionality, stores, and tests.

**Architecture:** Translate Tailwind v4 + shadcn-react design into Vue 3 + Tailwind v3.4 + shadcn-vue minimal subset. Replace tokens.css with oklch-based dual-theme system. Add Pinia tasks store for global active-task pill. Keep all existing Pinia stores, Vue Router, Tauri service wiring untouched. Move Shares from nav to a Dashboard "Create Shared DDC" card entry per design intent.

**Tech Stack:**
- Existing: Vue 3.4, Vite 5, Tailwind 3.4, Pinia 2, Vue Router 4, Tauri 2, Vitest 1.6
- New deps: `@vueuse/core` (theme + system query), `reka-ui` (Radix-Vue under shadcn-vue), `class-variance-authority`, `clsx`, `tailwind-merge`, `tailwindcss-animate`
- Webfont: Manrope Variable only (Inter retained as Windows fallback)

**Decisions baked in:**
- Shares cut from nav, accessed via Dashboard "Create Shared DDC" card (existing `ShareCreateWizard.vue` reused)
- shadcn-vue minimal: only `Button`, `Input`, `DropdownMenu` ported; everything else is hand-written Vue
- Tasks store built up-front (mock data) so global active-task pill works from day one
- Icon filenames standardized to Lucide canonical kebab-case
- `--font-sans` stack: `Inter, "Helvetica Neue", Helvetica, Arial, sans-serif`

---

## File Structure

### New / Modified Files

```
src/
  assets/
    fonts/
      Manrope-Variable.woff2          (NEW — Google Fonts)
      Inter-Variable.woff2            (KEPT — fallback for Windows)
      JetBrainsMono-Variable.woff2    (DELETED)
    icons/
      <28 new lucide SVGs>            (NEW)
      alert-triangle.svg              (RENAMED from triangle-alert.svg)
      help-circle.svg                 (RENAMED from circle-help.svg)
      x-circle.svg                    (RENAMED from circle-x.svg)
  styles/
    tokens.css                        (REWRITTEN — oklch dual-theme)
  style.css                            (MODIFIED — html.dark default)
  lib/
    utils.ts                          (NEW — cn helper)
    mockData.ts                       (NEW — ported from uecm-data.ts)
  composables/
    useColorMode.ts                   (NEW — light/dark/system)
  stores/
    tasks.ts                          (NEW — Pinia, active+history mocks)
    cluster.ts                        (NEW — Pinia, sidebar/topbar summary)
  components/
    ui/                                (NEW — shadcn-vue minimal port)
      Button.vue
      Input.vue
      dropdown-menu/
        index.ts
        DropdownMenu.vue
        DropdownMenuTrigger.vue
        DropdownMenuContent.vue
        DropdownMenuItem.vue
        DropdownMenuLabel.vue
        DropdownMenuSeparator.vue
    primitives/
      types.ts                         (MODIFIED — new tone names)
      UecmIcon.vue                     (KEPT — minor stroke fix)
      UecmStatusDot.vue                (REWRITTEN — new tones)
      UecmStatusBadge.vue              (NEW — icon + label, 3 sizes)
      UecmMatrixCell.vue               (NEW — health check matrix cell)
      UecmPageHeader.vue               (NEW — crumbs + title + actions)
      UecmFilterChip.vue               (NEW — `LABEL: VALUE ▾`)
      UecmStat.vue                     (NEW — Dashboard stat tile)
      UecmKpiTile.vue                  (NEW — Health Check KPI strip)
      UecmScoreTile.vue                (NEW — Cluster Score)
      UecmAlertRow.vue                 (NEW — Dashboard alert row)
      UecmArtifactPill.vue             (NEW — Project DDC/PSO pill)
      UecmCopyText.vue                 (NEW — copyable mono text)
      UecmKV.vue                       (REWRITTEN — uppercase mono key)
      UecmDetailCard.vue               (NEW — Machines/Projects card)
      UecmSectionHeader.vue            (NEW — section title + count)
      UecmCodeBlock.vue                (NEW — INI diff block)
      UecmThemeToggle.vue              (NEW — light/dark/system dropdown)
      UecmRoleTag.vue                  (NEW — Host/Render/Editor/Dev)
      (DELETED) UecmBtn.vue            (replaced by ui/Button.vue)
      (DELETED) UecmPill.vue           (replaced by UecmStatusBadge)
      (DELETED) UecmCard.vue           (use shadcn Card pattern inline)
      (DELETED) UecmField.vue          (unused)
      (DELETED) UecmTopBar.vue         (replaced by shell/UecmTopBar.vue)
      (DELETED) UecmInput.vue          (replaced by ui/Input.vue)
    shell/
      AppShell.vue                     (REWRITTEN — adds LogPanel)
      UecmSidebar.vue                  (NEW — replaces ActivityBar)
      UecmTopBar.vue                   (NEW — search + active task + cluster)
      UecmLogPanel.vue                 (NEW — sliding bottom drawer)
      ActivityBar.vue                  (DELETED)
    modals/
      BaseModal.vue                    (MODIFIED — accept size prop)
      <8 modals>                       (RESTYLED — new tokens, same selectors)
  views/
    Dashboard.vue                      (REWRITTEN)
    Machines.vue                       (REWRITTEN — keep store + selectors)
    Projects.vue                       (REWRITTEN)
    DDCPak.vue                         (REWRITTEN — adds 8-step Wizard)
    PSOCache.vue                       (REWRITTEN)
    INIScanner.vue                     (REWRITTEN — adds diff)
    HealthCheck.vue                    (REWRITTEN — Matrix + Console)
    Shares.vue                         (KEPT — accessible via Dashboard card)
  __tests__/
    AppShell.spec.ts                   (MODIFIED — 7 nav items)
    useColorMode.spec.ts               (NEW)
    tasks-store.spec.ts                (NEW)
    cluster-store.spec.ts              (NEW)
    UecmStatusBadge.spec.ts            (NEW)
    UecmThemeToggle.spec.ts            (NEW)

tailwind.config.js                     (REWRITTEN — v4→v3 translation)
package.json                           (MODIFIED — new deps)
```

### Deleted Files Summary

- `src/components/primitives/UecmBtn.vue`
- `src/components/primitives/UecmPill.vue`
- `src/components/primitives/UecmCard.vue`
- `src/components/primitives/UecmField.vue`
- `src/components/primitives/UecmTopBar.vue`
- `src/components/primitives/UecmInput.vue`
- `src/components/shell/ActivityBar.vue`
- `src/assets/fonts/JetBrainsMono-Variable.woff2`
- `src/assets/icons/triangle-alert.svg` (renamed)
- `src/assets/icons/circle-help.svg` (renamed)
- `src/assets/icons/circle-x.svg` (renamed)

---

## Phase 1 — Foundation Reset (45 min)

Replace tokens, fonts, Tailwind config, and icons. After this phase, the app builds with the new token system but visually still looks like the old design until later phases.

### Task 1.1: Download Manrope variable font

**Files:**
- Create: `src/assets/fonts/Manrope-Variable.woff2`

- [ ] **Step 1: Download Manrope from Google Fonts**

```bash
curl -L -o src/assets/fonts/Manrope-Variable.woff2 \
  "https://fonts.gstatic.com/s/manrope/v15/xn7gYHE41ni1AdIRggOxSvfedN4.woff2"
ls -la src/assets/fonts/Manrope-Variable.woff2
```

Expected: file ~40 KB.

- [ ] **Step 2: Verify file is valid woff2**

```bash
file src/assets/fonts/Manrope-Variable.woff2
```

Expected: `Web Open Font Format (Version 2)` or similar.

### Task 1.2: Delete unused JetBrains Mono font

**Files:**
- Delete: `src/assets/fonts/JetBrainsMono-Variable.woff2`

- [ ] **Step 1: Delete the file**

```bash
rm src/assets/fonts/JetBrainsMono-Variable.woff2
ls src/assets/fonts/
```

Expected: only `Inter-Variable.woff2` and `Manrope-Variable.woff2` remain.

### Task 1.3: Rename three legacy icons to Lucide canonical names

**Files:**
- Rename: `src/assets/icons/triangle-alert.svg` → `alert-triangle.svg`
- Rename: `src/assets/icons/circle-help.svg` → `help-circle.svg`
- Rename: `src/assets/icons/circle-x.svg` → `x-circle.svg`

- [ ] **Step 1: Rename**

```bash
cd src/assets/icons
mv triangle-alert.svg alert-triangle.svg
mv circle-help.svg help-circle.svg
mv circle-x.svg x-circle.svg
ls | grep -E "alert-triangle|help-circle|x-circle"
```

Expected: three lines showing the new names.

### Task 1.4: Add 28 missing Lucide icons

**Files:**
- Create: 28 new SVGs under `src/assets/icons/`

- [ ] **Step 1: Bulk-download from lucide repo**

```bash
ICONS="activity arrow-up-down check-circle-2 chevron-left circuit-board database edit-3 file-down file-text filter folder folder-open lightbulb loader-2 pause-circle play-circle power-off rotate-ccw scroll-text send shield shield-check skull wand-2 wrench x-octagon"
cd src/assets/icons
for name in $ICONS; do
  curl -fsSL -o "${name}.svg" "https://raw.githubusercontent.com/lucide-icons/lucide/main/icons/${name}.svg" || echo "FAILED: $name"
done
ls *.svg | wc -l
```

Expected: 65 icons total (37 existing + 28 new).

- [ ] **Step 2: Verify each new icon is valid SVG**

```bash
for name in $ICONS; do
  head -1 "${name}.svg" | grep -q "<svg" || echo "INVALID: $name"
done
echo "DONE"
```

Expected: only `DONE` printed (no `INVALID:` lines).

- [ ] **Step 3: Commit foundation file moves**

```bash
git add src/assets/fonts src/assets/icons
git commit -m "chore: rename icons to lucide canonical names + add 28 new icons + manrope font"
```

### Task 1.5: Rewrite tokens.css with oklch dual-theme system

**Files:**
- Modify: `src/styles/tokens.css` (full replace)

- [ ] **Step 1: Read current file then replace**

Read `src/styles/tokens.css` to see current content (already done in this conversation, but confirm before replacing).

- [ ] **Step 2: Replace contents**

```css
/* UECM design tokens — light + dark, oklch-based.
   Light = paper / blueprint feel. Dark = engineering operator console. */

@font-face {
  font-family: "Manrope";
  src: url("../assets/fonts/Manrope-Variable.woff2") format("woff2-variations");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Inter";
  src: url("../assets/fonts/Inter-Variable.woff2") format("woff2-variations");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}

:root {
  /* Light — paper / blueprint feel */
  --background: oklch(0.985 0.002 240);
  --foreground: oklch(0.22 0.01 240);
  --card: oklch(1 0 0);
  --card-foreground: oklch(0.22 0.01 240);
  --popover: oklch(1 0 0);
  --popover-foreground: oklch(0.22 0.01 240);
  --primary: oklch(0.58 0.13 210);
  --primary-foreground: oklch(0.99 0 0);
  --secondary: oklch(0.95 0.005 240);
  --secondary-foreground: oklch(0.22 0.01 240);
  --muted: oklch(0.955 0.005 240);
  --muted-foreground: oklch(0.5 0.01 240);
  --accent: oklch(0.93 0.008 220);
  --accent-foreground: oklch(0.22 0.01 240);
  --destructive: oklch(0.55 0.22 25);
  --destructive-foreground: oklch(0.99 0 0);
  --border: oklch(0.9 0.005 240);
  --input: oklch(0.9 0.005 240);
  --ring: oklch(0.58 0.13 210);
  --status-healthy: oklch(0.55 0.15 152);
  --status-warning: oklch(0.65 0.16 75);
  --status-critical: oklch(0.55 0.22 25);
  --status-info: oklch(0.58 0.13 210);
  --status-offline: oklch(0.65 0.005 240);
  --status-unknown: oklch(0.7 0.008 240);
  --sidebar: oklch(0.97 0.005 240);
  --sidebar-foreground: oklch(0.25 0.01 240);
  --sidebar-primary: oklch(0.58 0.13 210);
  --sidebar-primary-foreground: oklch(0.99 0 0);
  --sidebar-accent: oklch(0.93 0.008 220);
  --sidebar-accent-foreground: oklch(0.22 0.01 240);
  --sidebar-border: oklch(0.9 0.005 240);
  --radius: 0.5rem;
  --font-sans: "Inter", "Helvetica Neue", Helvetica, Arial, sans-serif;
  --font-display: "Manrope", "Inter", "Helvetica Neue", Helvetica, sans-serif;
  --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
}

.dark {
  --background: oklch(0.16 0.005 240);
  --foreground: oklch(0.96 0 0);
  --card: oklch(0.195 0.005 240);
  --card-foreground: oklch(0.96 0 0);
  --popover: oklch(0.205 0.005 240);
  --popover-foreground: oklch(0.96 0 0);
  --primary: oklch(0.78 0.13 200);
  --primary-foreground: oklch(0.16 0.005 240);
  --secondary: oklch(0.245 0.005 240);
  --secondary-foreground: oklch(0.96 0 0);
  --muted: oklch(0.235 0.005 240);
  --muted-foreground: oklch(0.68 0.01 240);
  --accent: oklch(0.27 0.005 240);
  --accent-foreground: oklch(0.96 0 0);
  --destructive: oklch(0.62 0.22 25);
  --destructive-foreground: oklch(0.98 0 0);
  --border: oklch(0.27 0.005 240);
  --input: oklch(0.27 0.005 240);
  --ring: oklch(0.78 0.13 200);
  --status-healthy: oklch(0.72 0.16 152);
  --status-warning: oklch(0.78 0.16 75);
  --status-critical: oklch(0.65 0.22 25);
  --status-info: oklch(0.78 0.13 200);
  --status-offline: oklch(0.5 0.005 240);
  --status-unknown: oklch(0.6 0.01 240);
  --sidebar: oklch(0.175 0.005 240);
  --sidebar-foreground: oklch(0.92 0 0);
  --sidebar-primary: oklch(0.78 0.13 200);
  --sidebar-primary-foreground: oklch(0.16 0.005 240);
  --sidebar-accent: oklch(0.245 0.005 240);
  --sidebar-accent-foreground: oklch(0.96 0 0);
  --sidebar-border: oklch(0.25 0.005 240);
}

html, body {
  background: var(--background);
  color: var(--foreground);
  font-family: var(--font-sans);
  font-feature-settings: "cv11", "ss01", "ss03";
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

.font-display { font-family: var(--font-display); }
.font-mono { font-family: var(--font-mono); }

/* Hatched pattern utility for unknown / stale cells */
.bg-hatched {
  background-image: repeating-linear-gradient(
    45deg,
    color-mix(in oklch, var(--muted-foreground) 25%, transparent) 0 2px,
    transparent 2px 6px
  );
}
```

- [ ] **Step 3: Verify file written correctly**

```bash
head -5 src/styles/tokens.css
wc -l src/styles/tokens.css
```

Expected: starts with `/* UECM design tokens` and is ~110 lines.

### Task 1.6: Install tailwindcss-animate

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Install**

```bash
pnpm add -D tailwindcss-animate
```

Expected: package added without conflicts.

### Task 1.7: Rewrite tailwind.config.js

**Files:**
- Modify: `tailwind.config.js` (full replace)

- [ ] **Step 1: Replace contents**

```js
import animate from "tailwindcss-animate";

/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{vue,ts,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "var(--background)",
        foreground: "var(--foreground)",
        card: {
          DEFAULT: "var(--card)",
          foreground: "var(--card-foreground)",
        },
        popover: {
          DEFAULT: "var(--popover)",
          foreground: "var(--popover-foreground)",
        },
        primary: {
          DEFAULT: "var(--primary)",
          foreground: "var(--primary-foreground)",
        },
        secondary: {
          DEFAULT: "var(--secondary)",
          foreground: "var(--secondary-foreground)",
        },
        muted: {
          DEFAULT: "var(--muted)",
          foreground: "var(--muted-foreground)",
        },
        accent: {
          DEFAULT: "var(--accent)",
          foreground: "var(--accent-foreground)",
        },
        destructive: {
          DEFAULT: "var(--destructive)",
          foreground: "var(--destructive-foreground)",
        },
        sidebar: {
          DEFAULT: "var(--sidebar)",
          foreground: "var(--sidebar-foreground)",
          primary: "var(--sidebar-primary)",
          "primary-foreground": "var(--sidebar-primary-foreground)",
          accent: "var(--sidebar-accent)",
          "accent-foreground": "var(--sidebar-accent-foreground)",
          border: "var(--sidebar-border)",
        },
        status: {
          healthy: "var(--status-healthy)",
          warning: "var(--status-warning)",
          critical: "var(--status-critical)",
          info: "var(--status-info)",
          offline: "var(--status-offline)",
          unknown: "var(--status-unknown)",
        },
      },
      borderColor: {
        DEFAULT: "var(--border)",
      },
      borderRadius: {
        sm: "calc(var(--radius) - 4px)",
        DEFAULT: "calc(var(--radius) - 2px)",
        md: "calc(var(--radius) - 2px)",
        lg: "var(--radius)",
        xl: "calc(var(--radius) + 4px)",
      },
      fontFamily: {
        sans: "var(--font-sans)",
        display: "var(--font-display)",
        mono: "var(--font-mono)",
      },
      ringColor: {
        DEFAULT: "var(--ring)",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [animate],
};
```

- [ ] **Step 2: Verify build succeeds**

```bash
pnpm build 2>&1 | tail -10
```

Expected: build succeeds; no errors about missing tokens or invalid Tailwind config.

### Task 1.8: Update style.css to keep import order + set default dark

**Files:**
- Modify: `src/style.css`

- [ ] **Step 1: Replace contents**

```css
@import "./styles/tokens.css";

@tailwind base;
@tailwind components;
@tailwind utilities;

html, body, #app {
  height: 100%;
  margin: 0;
  padding: 0;
}
```

- [ ] **Step 2: Set default dark on `<html>`**

Modify `index.html`. Change:

```html
<html lang="en">
```

to:

```html
<html lang="en" class="dark">
```

- [ ] **Step 3: Verify build still passes**

```bash
pnpm build 2>&1 | tail -5
```

Expected: build succeeds.

### Task 1.9: Delete obsolete primitives now that we'll replace them

**Files:**
- Delete: `src/components/primitives/UecmBtn.vue`
- Delete: `src/components/primitives/UecmPill.vue`
- Delete: `src/components/primitives/UecmCard.vue`
- Delete: `src/components/primitives/UecmField.vue`
- Delete: `src/components/primitives/UecmTopBar.vue`
- Delete: `src/components/primitives/UecmInput.vue`

- [ ] **Step 1: Verify nothing else imports them**

```bash
grep -rn "UecmBtn\|UecmPill\|UecmCard\|UecmField\|UecmTopBar\|UecmInput" src/ --exclude-dir=primitives
```

Expected: empty (no other importers).

- [ ] **Step 2: Delete the files and update barrel index**

```bash
rm src/components/primitives/UecmBtn.vue \
   src/components/primitives/UecmPill.vue \
   src/components/primitives/UecmCard.vue \
   src/components/primitives/UecmField.vue \
   src/components/primitives/UecmTopBar.vue \
   src/components/primitives/UecmInput.vue
```

- [ ] **Step 3: Update `src/components/primitives/index.ts`**

Replace contents:

```ts
export { default as UecmIcon } from "./UecmIcon.vue";
export { default as UecmStatusDot } from "./UecmStatusDot.vue";
export { default as UecmKV } from "./UecmKV.vue";
```

- [ ] **Step 4: Verify build**

```bash
pnpm build 2>&1 | tail -5
```

Expected: passes (or fails only on imports inside views/modals — those will be fixed in later phases).

### Task 1.10: Verify Phase 1 — tests still pass

- [ ] **Step 1: Run full test suite**

```bash
pnpm test 2>&1 | tail -8
```

Expected: most tests pass. Some tests that depend on deleted primitives (UecmBtn etc.) may break — that's OK, those primitives weren't exposed in tests yet (they were only used inside ActivityBar/AppShell which we'll rebuild). If unexpected breakage, list and address before commit.

- [ ] **Step 2: Commit Phase 1**

```bash
git add -A
git commit -m "feat(design-system): replace tokens.css with oklch dual-theme + tailwind v4→v3 + delete legacy primitives"
```

---

## Phase 2 — Theme Switching (30 min)

Add light/dark/system theme switching using `@vueuse/core`.

### Task 2.1: Install @vueuse/core

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Install**

```bash
pnpm add @vueuse/core
```

### Task 2.2: TDD useColorMode composable

**Files:**
- Create: `src/composables/useColorMode.ts`
- Test: `src/__tests__/useColorMode.spec.ts`

- [ ] **Step 1: Write failing test**

```ts
// src/__tests__/useColorMode.spec.ts
import { describe, it, expect, beforeEach } from "vitest";
import { nextTick } from "vue";
import { useColorMode } from "@/composables/useColorMode";

describe("useColorMode", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.className = "";
  });

  it("defaults to dark when nothing stored", () => {
    const { mode, resolved } = useColorMode();
    expect(mode.value).toBe("dark");
    expect(resolved.value).toBe("dark");
  });

  it("applies dark class on <html> when set to dark", async () => {
    const { mode } = useColorMode();
    mode.value = "dark";
    await nextTick();
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("removes dark class when set to light", async () => {
    const { mode } = useColorMode();
    mode.value = "light";
    await nextTick();
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("persists mode to localStorage", () => {
    const { mode } = useColorMode();
    mode.value = "light";
    expect(localStorage.getItem("uecm-theme")).toBe("light");
  });

  it("system mode resolves to OS preference", () => {
    const { mode, resolved } = useColorMode();
    mode.value = "system";
    expect(["light", "dark"]).toContain(resolved.value);
  });
});
```

- [ ] **Step 2: Run, see fail**

```bash
pnpm vitest run src/__tests__/useColorMode.spec.ts
```

Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

```ts
// src/composables/useColorMode.ts
import { useColorMode as useVueUseColorMode } from "@vueuse/core";
import { computed } from "vue";

export type ThemeMode = "light" | "dark" | "system";

export function useColorMode() {
  const colorMode = useVueUseColorMode({
    storageKey: "uecm-theme",
    initialValue: "dark",
    attribute: "class",
    selector: "html",
    modes: { dark: "dark", light: "" },
  });

  const mode = computed<ThemeMode>({
    get: () => (colorMode.value === "auto" ? "system" : (colorMode.value as ThemeMode)),
    set: (v) => {
      colorMode.value = v === "system" ? "auto" : v;
    },
  });

  const resolved = computed<"light" | "dark">(() => {
    if (mode.value !== "system") return mode.value;
    if (typeof window === "undefined") return "dark";
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  });

  return { mode, resolved };
}
```

- [ ] **Step 4: Run, see pass**

```bash
pnpm vitest run src/__tests__/useColorMode.spec.ts
```

Expected: 5 PASS.

- [ ] **Step 5: Commit**

```bash
git add src/composables src/__tests__/useColorMode.spec.ts package.json pnpm-lock.yaml
git commit -m "feat(theme): add useColorMode composable with light/dark/system + persistence"
```

---

## Phase 3 — shadcn-vue Minimal Port (40 min)

Port only the 3 shadcn components actually used: `Button`, `Input`, `DropdownMenu`. Use Reka UI (the Vue equivalent of Radix).

### Task 3.1: Install Reka UI + cn helper deps

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Install runtime deps**

```bash
pnpm add reka-ui class-variance-authority clsx tailwind-merge
```

Expected: 4 packages installed.

### Task 3.2: Create cn utility

**Files:**
- Create: `src/lib/utils.ts`

- [ ] **Step 1: Write file**

```ts
// src/lib/utils.ts
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

### Task 3.3: Port shadcn-vue Button

**Files:**
- Create: `src/components/ui/Button.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cva, type VariantProps } from "class-variance-authority";
import { Primitive, type PrimitiveProps } from "reka-ui";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md font-display text-sm font-bold tracking-tight transition-all disabled:pointer-events-none disabled:opacity-50 shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        destructive:
          "bg-destructive text-white hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/60",
        outline:
          "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground dark:bg-input/30 dark:border-input dark:hover:bg-input/50",
        secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
        ghost: "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 rounded-md gap-1.5 px-3",
        lg: "h-10 rounded-md px-6",
        icon: "size-9",
        "icon-sm": "size-8",
        "icon-lg": "size-10",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
);

type Variants = VariantProps<typeof buttonVariants>;

const props = withDefaults(
  defineProps<{
    variant?: Variants["variant"];
    size?: Variants["size"];
    asChild?: PrimitiveProps["asChild"];
    as?: PrimitiveProps["as"];
    class?: string;
  }>(),
  {
    as: "button",
    asChild: false,
  }
);

const cls = computed(() =>
  cn(buttonVariants({ variant: props.variant, size: props.size }), props.class)
);
</script>

<template>
  <Primitive :as="as" :as-child="asChild" :class="cls">
    <slot />
  </Primitive>
</template>
```

- [ ] **Step 2: Verify build**

```bash
pnpm build 2>&1 | tail -5
```

Expected: passes.

### Task 3.4: Port shadcn-vue Input

**Files:**
- Create: `src/components/ui/Input.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/utils";

const props = withDefaults(
  defineProps<{
    modelValue?: string | number;
    type?: string;
    class?: string;
  }>(),
  {
    type: "text",
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const cls = computed(() =>
  cn(
    "file:text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground dark:bg-input/30 border-input flex h-9 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
    "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]",
    "aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
    props.class
  )
);
</script>

<template>
  <input
    :type="type"
    :value="modelValue"
    :class="cls"
    @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
  />
</template>
```

### Task 3.5: Port shadcn-vue DropdownMenu (sub-files)

**Files:**
- Create: `src/components/ui/dropdown-menu/DropdownMenu.vue`
- Create: `src/components/ui/dropdown-menu/DropdownMenuTrigger.vue`
- Create: `src/components/ui/dropdown-menu/DropdownMenuContent.vue`
- Create: `src/components/ui/dropdown-menu/DropdownMenuItem.vue`
- Create: `src/components/ui/dropdown-menu/DropdownMenuLabel.vue`
- Create: `src/components/ui/dropdown-menu/DropdownMenuSeparator.vue`
- Create: `src/components/ui/dropdown-menu/index.ts`

- [ ] **Step 1: DropdownMenu.vue**

```vue
<script setup lang="ts">
import { DropdownMenuRoot, type DropdownMenuRootEmits, type DropdownMenuRootProps, useForwardPropsEmits } from "reka-ui";

const props = defineProps<DropdownMenuRootProps>();
const emits = defineEmits<DropdownMenuRootEmits>();
const forwarded = useForwardPropsEmits(props, emits);
</script>

<template>
  <DropdownMenuRoot v-bind="forwarded">
    <slot />
  </DropdownMenuRoot>
</template>
```

- [ ] **Step 2: DropdownMenuTrigger.vue**

```vue
<script setup lang="ts">
import { DropdownMenuTrigger, type DropdownMenuTriggerProps } from "reka-ui";

const props = defineProps<DropdownMenuTriggerProps>();
</script>

<template>
  <DropdownMenuTrigger v-bind="props">
    <slot />
  </DropdownMenuTrigger>
</template>
```

- [ ] **Step 3: DropdownMenuContent.vue**

```vue
<script setup lang="ts">
import { computed } from "vue";
import {
  DropdownMenuContent,
  DropdownMenuPortal,
  type DropdownMenuContentEmits,
  type DropdownMenuContentProps,
  useForwardPropsEmits,
} from "reka-ui";
import { cn } from "@/lib/utils";

const props = withDefaults(defineProps<DropdownMenuContentProps & { class?: string }>(), {
  sideOffset: 4,
});
const emits = defineEmits<DropdownMenuContentEmits>();

const delegated = computed(() => {
  const { class: _, ...rest } = props;
  return rest;
});
const forwarded = useForwardPropsEmits(delegated, emits);
</script>

<template>
  <DropdownMenuPortal>
    <DropdownMenuContent
      v-bind="forwarded"
      :class="cn('z-50 min-w-[8rem] overflow-hidden rounded-md border bg-popover p-1 text-popover-foreground shadow-md data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2', props.class)"
    >
      <slot />
    </DropdownMenuContent>
  </DropdownMenuPortal>
</template>
```

- [ ] **Step 4: DropdownMenuItem.vue**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { DropdownMenuItem, type DropdownMenuItemProps, useForwardProps } from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<DropdownMenuItemProps & { class?: string; inset?: boolean }>();
const delegated = computed(() => {
  const { class: _, inset: __, ...rest } = props;
  return rest;
});
const forwarded = useForwardProps(delegated);
</script>

<template>
  <DropdownMenuItem
    v-bind="forwarded"
    :data-inset="inset || undefined"
    :class="cn('relative flex cursor-default select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[inset]:pl-8', props.class)"
  >
    <slot />
  </DropdownMenuItem>
</template>
```

- [ ] **Step 5: DropdownMenuLabel.vue**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { DropdownMenuLabel, type DropdownMenuLabelProps, useForwardProps } from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<DropdownMenuLabelProps & { class?: string }>();
const delegated = computed(() => {
  const { class: _, ...rest } = props;
  return rest;
});
const forwarded = useForwardProps(delegated);
</script>

<template>
  <DropdownMenuLabel v-bind="forwarded" :class="cn('px-2 py-1.5 text-sm font-semibold', props.class)">
    <slot />
  </DropdownMenuLabel>
</template>
```

- [ ] **Step 6: DropdownMenuSeparator.vue**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { DropdownMenuSeparator, type DropdownMenuSeparatorProps, useForwardProps } from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<DropdownMenuSeparatorProps & { class?: string }>();
const delegated = computed(() => {
  const { class: _, ...rest } = props;
  return rest;
});
const forwarded = useForwardProps(delegated);
</script>

<template>
  <DropdownMenuSeparator v-bind="forwarded" :class="cn('-mx-1 my-1 h-px bg-muted', props.class)" />
</template>
```

- [ ] **Step 7: dropdown-menu/index.ts**

```ts
export { default as DropdownMenu } from "./DropdownMenu.vue";
export { default as DropdownMenuTrigger } from "./DropdownMenuTrigger.vue";
export { default as DropdownMenuContent } from "./DropdownMenuContent.vue";
export { default as DropdownMenuItem } from "./DropdownMenuItem.vue";
export { default as DropdownMenuLabel } from "./DropdownMenuLabel.vue";
export { default as DropdownMenuSeparator } from "./DropdownMenuSeparator.vue";
```

- [ ] **Step 8: Verify build**

```bash
pnpm build 2>&1 | tail -10
```

Expected: passes; no errors.

- [ ] **Step 9: Commit**

```bash
git add src/lib src/components/ui package.json pnpm-lock.yaml
git commit -m "feat(ui): port shadcn-vue Button + Input + DropdownMenu (reka-ui)"
```

---

## Phase 4 — UECM Theme Toggle (15 min)

### Task 4.1: TDD UecmThemeToggle component

**Files:**
- Create: `src/components/primitives/UecmThemeToggle.vue`
- Test: `src/__tests__/UecmThemeToggle.spec.ts`

- [ ] **Step 1: Write failing test**

```ts
// src/__tests__/UecmThemeToggle.spec.ts
import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import UecmThemeToggle from "@/components/primitives/UecmThemeToggle.vue";

describe("UecmThemeToggle", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.className = "";
  });

  it("renders the toggle button", () => {
    const wrapper = mount(UecmThemeToggle);
    expect(wrapper.find("[data-theme-toggle]").exists()).toBe(true);
  });

  it("has aria-label for accessibility", () => {
    const wrapper = mount(UecmThemeToggle);
    expect(wrapper.find("[data-theme-toggle]").attributes("aria-label")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run, see fail**

```bash
pnpm vitest run src/__tests__/UecmThemeToggle.spec.ts
```

Expected: FAIL — component not found.

- [ ] **Step 3: Implement**

```vue
<!-- src/components/primitives/UecmThemeToggle.vue -->
<script setup lang="ts">
import { useColorMode, type ThemeMode } from "@/composables/useColorMode";
import { Button } from "@/components/ui/Button.vue";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import UecmIcon from "./UecmIcon.vue";

const { mode, resolved } = useColorMode();

const OPTIONS: { value: ThemeMode; label: string; hint: string; icon: string }[] = [
  { value: "light", label: "浅色", hint: "Light", icon: "sun" },
  { value: "dark", label: "深色", hint: "Dark", icon: "moon" },
  { value: "system", label: "跟随系统", hint: "System", icon: "monitor" },
];
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <button
        data-theme-toggle
        aria-label="切换主题"
        class="relative inline-flex h-8 w-8 items-center justify-center rounded-md hover:bg-accent text-foreground transition-colors"
      >
        <UecmIcon
          :name="resolved === 'dark' ? 'moon' : 'sun'"
          :size="16"
        />
        <span
          v-if="mode === 'system'"
          aria-hidden="true"
          class="pointer-events-none absolute right-1 top-1 h-1.5 w-1.5 rounded-full bg-status-info ring-2 ring-card"
        />
      </button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="w-44">
      <DropdownMenuLabel class="text-[10px] font-mono uppercase tracking-[0.14em] text-muted-foreground">
        Appearance
      </DropdownMenuLabel>
      <DropdownMenuSeparator />
      <DropdownMenuItem
        v-for="opt in OPTIONS"
        :key="opt.value"
        class="flex items-center gap-2"
        @select="mode = opt.value"
      >
        <UecmIcon :name="opt.icon" :size="14" class="text-muted-foreground" />
        <span class="flex-1 text-sm">{{ opt.label }}</span>
        <span class="font-mono text-[10px] text-muted-foreground">{{ opt.hint }}</span>
        <UecmIcon
          v-if="mode === opt.value"
          name="check"
          :size="14"
          class="text-status-info"
        />
        <span v-else class="h-3.5 w-3.5" aria-hidden="true" />
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
```

- [ ] **Step 4: Add `sun`, `moon`, `monitor` icons (not in our 65)**

```bash
cd src/assets/icons
for name in sun moon monitor; do
  curl -fsSL -o "${name}.svg" "https://raw.githubusercontent.com/lucide-icons/lucide/main/icons/${name}.svg"
done
ls sun.svg moon.svg monitor.svg
```

- [ ] **Step 5: Run tests**

```bash
pnpm vitest run src/__tests__/UecmThemeToggle.spec.ts
```

Expected: 2 PASS.

- [ ] **Step 6: Commit**

```bash
git add src/components/primitives/UecmThemeToggle.vue src/assets/icons/sun.svg src/assets/icons/moon.svg src/assets/icons/monitor.svg src/__tests__/UecmThemeToggle.spec.ts
git commit -m "feat(theme): add UecmThemeToggle with light/dark/system dropdown"
```

---

## Phase 5 — Mock Data + Stores (45 min)

### Task 5.1: Port mock data

**Files:**
- Create: `src/lib/mockData.ts`

- [ ] **Step 1: Read source mock data**

Read `/Users/bip.lan/Downloads/b_sVGHApfkUd9/lib/uecm-data.ts` (already read in this conversation; ~600 lines).

- [ ] **Step 2: Port to TypeScript**

Create `src/lib/mockData.ts` with the same exports adapted to our codebase:
- `StatusKind`, `MachineRole`, `Machine`, `OperationRecord`, `Project`, `TaskItem`, `IniFinding` types
- `HEALTH_CHECKS`, `MACHINES`, `RECENT_OPS`, `PROJECTS`, `RUNNING_TASKS`, `HISTORY_TASKS`, `INI_FINDINGS`, `LOG_LINES`
- Derived: `TOTAL`, `ONLINE`, `HEALTHY_COUNT`, `WARNING_COUNT`, `CRITICAL_COUNT`, `CLUSTER_HEALTH_PCT`

Use the exact same data values and same shape. Copy entirely from `/Users/bip.lan/Downloads/b_sVGHApfkUd9/lib/uecm-data.ts` (the file is 651 lines; preserve verbatim, only changing the file header comment if needed).

Do NOT shorten or summarize the data — every machine, project, finding, task, and log line must be ported exactly. The mock data is part of the design contract.

- [ ] **Step 3: Verify it compiles**

```bash
pnpm build 2>&1 | tail -5
```

Expected: passes.

### Task 5.2: TDD tasks store

**Files:**
- Create: `src/stores/tasks.ts`
- Test: `src/__tests__/tasks-store.spec.ts`

- [ ] **Step 1: Write failing test**

```ts
// src/__tests__/tasks-store.spec.ts
import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useTasksStore } from "@/stores/tasks";

describe("tasksStore", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("loads mock running tasks on init", () => {
    const store = useTasksStore();
    expect(store.running.length).toBeGreaterThan(0);
  });

  it("loads mock history", () => {
    const store = useTasksStore();
    expect(store.history.length).toBeGreaterThan(0);
  });

  it("activeTask returns first running task or null", () => {
    const store = useTasksStore();
    expect(store.activeTask).toBe(store.running[0]);
  });

  it("pauseTask flips status (mock-only)", () => {
    const store = useTasksStore();
    const id = store.running[0].id;
    store.pauseTask(id);
    expect(store.running.find((t) => t.id === id)?.paused).toBe(true);
  });

  it("cancelTask removes from running", () => {
    const store = useTasksStore();
    const id = store.running[0].id;
    const initial = store.running.length;
    store.cancelTask(id);
    expect(store.running.length).toBe(initial - 1);
  });
});
```

- [ ] **Step 2: Run, see fail**

```bash
pnpm vitest run src/__tests__/tasks-store.spec.ts
```

Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

```ts
// src/stores/tasks.ts
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { RUNNING_TASKS, HISTORY_TASKS, type TaskItem } from "@/lib/mockData";

export interface RunningTask extends TaskItem {
  paused?: boolean;
}

export const useTasksStore = defineStore("tasks", () => {
  const running = ref<RunningTask[]>([...RUNNING_TASKS]);
  const history = ref<TaskItem[]>([...HISTORY_TASKS]);

  const activeTask = computed<RunningTask | null>(() => running.value[0] ?? null);

  function pauseTask(id: string) {
    const t = running.value.find((x) => x.id === id);
    if (t) t.paused = !t.paused;
  }

  function cancelTask(id: string) {
    running.value = running.value.filter((t) => t.id !== id);
  }

  return { running, history, activeTask, pauseTask, cancelTask };
});
```

- [ ] **Step 4: Run, see pass**

```bash
pnpm vitest run src/__tests__/tasks-store.spec.ts
```

Expected: 5 PASS.

### Task 5.3: TDD cluster store

**Files:**
- Create: `src/stores/cluster.ts`
- Test: `src/__tests__/cluster-store.spec.ts`

- [ ] **Step 1: Write failing test**

```ts
// src/__tests__/cluster-store.spec.ts
import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useClusterStore } from "@/stores/cluster";

describe("clusterStore", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("exposes total + online counts", () => {
    const store = useClusterStore();
    expect(store.total).toBe(10);
    expect(store.online).toBe(9);
  });

  it("exposes healthy/warning/critical/offline counts", () => {
    const store = useClusterStore();
    expect(store.healthyCount + store.warningCount + store.criticalCount).toBe(8);
    expect(store.offlineCount).toBe(1);
  });

  it("score is hardcoded mock 70 for sidebar/header", () => {
    const store = useClusterStore();
    expect(store.score).toBe(70);
  });

  it("status is degraded when score < 90", () => {
    const store = useClusterStore();
    expect(store.statusLabel).toBe("DEGRADED");
  });
});
```

- [ ] **Step 2: Run, see fail**

```bash
pnpm vitest run src/__tests__/cluster-store.spec.ts
```

Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
// src/stores/cluster.ts
import { defineStore } from "pinia";
import { computed } from "vue";
import {
  TOTAL,
  ONLINE,
  HEALTHY_COUNT,
  WARNING_COUNT,
  CRITICAL_COUNT,
} from "@/lib/mockData";

export const useClusterStore = defineStore("cluster", () => {
  const total = computed(() => TOTAL);
  const online = computed(() => ONLINE);
  const healthyCount = computed(() => HEALTHY_COUNT);
  const warningCount = computed(() => WARNING_COUNT);
  const criticalCount = computed(() => CRITICAL_COUNT);
  const offlineCount = computed(() => total.value - online.value);
  const score = computed(() => 70);
  const name = computed(() => "vp-stage-a");
  const version = computed(() => "v1.4.2");
  const statusLabel = computed(() => {
    if (score.value >= 90) return "HEALTHY";
    if (score.value >= 60) return "DEGRADED";
    return "CRITICAL";
  });

  return {
    total,
    online,
    healthyCount,
    warningCount,
    criticalCount,
    offlineCount,
    score,
    name,
    version,
    statusLabel,
  };
});
```

- [ ] **Step 4: Run, see pass**

```bash
pnpm vitest run src/__tests__/cluster-store.spec.ts
```

Expected: 4 PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/mockData.ts src/stores/tasks.ts src/stores/cluster.ts src/__tests__/tasks-store.spec.ts src/__tests__/cluster-store.spec.ts
git commit -m "feat(stores): add mockData + tasks + cluster Pinia stores"
```

---

## Phase 6 — UECM Primitives (90 min)

Build the new visual primitives that views/shell will use.

### Task 6.1: Update primitives/types.ts with new tone names

**Files:**
- Modify: `src/components/primitives/types.ts`

- [ ] **Step 1: Replace contents**

```ts
// src/components/primitives/types.ts
export type Tone =
  | "healthy"
  | "warning"
  | "critical"
  | "info"
  | "offline"
  | "na"
  | "unknown";
```

### Task 6.2: Rewrite UecmStatusDot

**Files:**
- Modify: `src/components/primitives/UecmStatusDot.vue`

- [ ] **Step 1: Replace contents**

```vue
<script setup lang="ts">
import type { Tone } from "./types";

const props = withDefaults(
  defineProps<{
    status?: Tone;
    size?: number;
    pulse?: boolean;
  }>(),
  {
    status: "unknown",
    size: 8,
    pulse: false,
  }
);

const TONE_BG: Record<Tone, string> = {
  healthy: "bg-status-healthy",
  warning: "bg-status-warning",
  critical: "bg-status-critical",
  info: "bg-status-info",
  offline: "bg-status-offline",
  na: "bg-muted-foreground",
  unknown: "bg-muted-foreground",
};
</script>

<template>
  <span
    :aria-label="status"
    :class="[
      'inline-block shrink-0 rounded-full align-middle',
      TONE_BG[status],
      pulse && 'animate-pulse',
    ]"
    :style="{ width: `${size}px`, height: `${size}px` }"
  />
</template>
```

### Task 6.3: TDD UecmStatusBadge (icon + label, 3 sizes)

**Files:**
- Create: `src/components/primitives/UecmStatusBadge.vue`
- Test: `src/__tests__/UecmStatusBadge.spec.ts`

- [ ] **Step 1: Write failing test**

```ts
// src/__tests__/UecmStatusBadge.spec.ts
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";

describe("UecmStatusBadge", () => {
  it("renders default label from tone", () => {
    const wrapper = mount(UecmStatusBadge, { props: { status: "healthy" } });
    expect(wrapper.text()).toContain("HEALTHY");
  });

  it("uses custom label when provided", () => {
    const wrapper = mount(UecmStatusBadge, {
      props: { status: "warning", label: "DEGRADED" },
    });
    expect(wrapper.text()).toContain("DEGRADED");
    expect(wrapper.text()).not.toContain("WARNING");
  });

  it("has tone-specific class for healthy", () => {
    const wrapper = mount(UecmStatusBadge, { props: { status: "healthy" } });
    expect(wrapper.classes().some((c) => c.includes("status-healthy"))).toBe(true);
  });

  it("supports xs/sm/md sizes", () => {
    const xs = mount(UecmStatusBadge, { props: { status: "healthy", size: "xs" } });
    expect(xs.classes()).toContain("h-5");
    const md = mount(UecmStatusBadge, { props: { status: "healthy", size: "md" } });
    expect(md.classes()).toContain("h-7");
  });
});
```

- [ ] **Step 2: Run, see fail**

```bash
pnpm vitest run src/__tests__/UecmStatusBadge.spec.ts
```

Expected: FAIL.

- [ ] **Step 3: Implement**

```vue
<!-- src/components/primitives/UecmStatusBadge.vue -->
<script setup lang="ts">
import { computed } from "vue";
import UecmIcon from "./UecmIcon.vue";
import type { Tone } from "./types";

interface ToneMeta {
  label: string;
  icon: string;
  spin?: boolean;
}

const TONE_META: Record<Tone, ToneMeta> = {
  healthy: { label: "HEALTHY", icon: "check-circle-2" },
  warning: { label: "WARNING", icon: "alert-triangle" },
  critical: { label: "CRITICAL", icon: "x-octagon" },
  info: { label: "INFO", icon: "loader-2", spin: true },
  na: { label: "N/A", icon: "minus" },
  offline: { label: "OFFLINE", icon: "power-off" },
  unknown: { label: "UNKNOWN", icon: "help-circle" },
};

const TONE_CLS: Record<Tone, string> = {
  healthy:
    "text-status-healthy bg-[color-mix(in_oklch,var(--status-healthy)_15%,transparent)] border-[color-mix(in_oklch,var(--status-healthy)_40%,transparent)]",
  warning:
    "text-status-warning bg-[color-mix(in_oklch,var(--status-warning)_15%,transparent)] border-[color-mix(in_oklch,var(--status-warning)_40%,transparent)]",
  critical:
    "text-status-critical bg-[color-mix(in_oklch,var(--status-critical)_15%,transparent)] border-[color-mix(in_oklch,var(--status-critical)_45%,transparent)]",
  info:
    "text-status-info bg-[color-mix(in_oklch,var(--status-info)_15%,transparent)] border-[color-mix(in_oklch,var(--status-info)_40%,transparent)]",
  offline:
    "text-status-offline bg-[color-mix(in_oklch,var(--status-offline)_18%,transparent)] border-[color-mix(in_oklch,var(--status-offline)_40%,transparent)]",
  na: "text-muted-foreground bg-muted/40 border-border",
  unknown: "text-muted-foreground bg-muted/30 border-border",
};

const props = withDefaults(
  defineProps<{
    status: Tone;
    label?: string;
    size?: "xs" | "sm" | "md";
  }>(),
  { size: "sm" }
);

const sizeCls = computed(() =>
  ({
    xs: "h-5 px-1.5 text-[11px] gap-1",
    sm: "h-6 px-2 text-[11px] gap-1.5",
    md: "h-7 px-2.5 text-xs gap-1.5",
  }[props.size])
);
const iconSize = computed(() => (props.size === "md" ? 14 : 12));
const meta = computed(() => TONE_META[props.status]);
</script>

<template>
  <span
    :class="[
      'inline-flex items-center rounded-md border font-medium tracking-wide',
      sizeCls,
      TONE_CLS[status],
    ]"
  >
    <UecmIcon
      :name="meta.icon"
      :size="iconSize"
      :class="meta.spin && 'animate-spin'"
    />
    <span class="font-mono uppercase">{{ label ?? meta.label }}</span>
  </span>
</template>
```

- [ ] **Step 4: Run, see pass**

```bash
pnpm vitest run src/__tests__/UecmStatusBadge.spec.ts
```

Expected: 4 PASS.

### Task 6.4: Build UecmMatrixCell

**Files:**
- Create: `src/components/primitives/UecmMatrixCell.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import { computed } from "vue";
import UecmIcon from "./UecmIcon.vue";
import type { Tone } from "./types";

const TONE_BG: Record<Tone, string> = {
  healthy: "bg-[color-mix(in_oklch,var(--status-healthy)_15%,transparent)] border-[color-mix(in_oklch,var(--status-healthy)_40%,transparent)] text-status-healthy",
  warning: "bg-[color-mix(in_oklch,var(--status-warning)_15%,transparent)] border-[color-mix(in_oklch,var(--status-warning)_40%,transparent)] text-status-warning",
  critical: "bg-[color-mix(in_oklch,var(--status-critical)_15%,transparent)] border-[color-mix(in_oklch,var(--status-critical)_45%,transparent)] text-status-critical",
  info: "bg-[color-mix(in_oklch,var(--status-info)_15%,transparent)] border-[color-mix(in_oklch,var(--status-info)_40%,transparent)] text-status-info",
  na: "bg-muted/40 border-border text-muted-foreground",
  offline: "bg-[color-mix(in_oklch,var(--status-offline)_18%,transparent)] border-[color-mix(in_oklch,var(--status-offline)_40%,transparent)] text-status-offline",
  unknown: "bg-muted/30 border-border text-muted-foreground",
};

const TONE_ICON: Record<Tone, string> = {
  healthy: "check-circle-2",
  warning: "alert-triangle",
  critical: "x-octagon",
  info: "loader-2",
  na: "minus",
  offline: "power-off",
  unknown: "help-circle",
};

const props = defineProps<{
  status: Tone;
  emphasized?: boolean;
  selected?: boolean;
}>();

const emit = defineEmits<{ click: [] }>();

const isHatched = computed(() => props.status === "unknown");
const isSpin = computed(() => props.status === "info");
</script>

<template>
  <button
    type="button"
    :aria-label="status"
    :class="[
      'group relative flex h-9 w-full items-center justify-center border-r border-b border-border/60 transition-colors',
      'hover:bg-accent/40',
      selected && 'ring-1 ring-primary ring-inset',
      isHatched && 'bg-hatched',
    ]"
    @click="emit('click')"
  >
    <span
      :class="[
        'flex h-7 w-7 items-center justify-center rounded-md border',
        TONE_BG[status],
        emphasized && 'ring-1 ring-offset-0 ring-foreground/20',
      ]"
    >
      <UecmIcon
        :name="TONE_ICON[status]"
        :size="14"
        :class="isSpin && 'animate-spin'"
      />
    </span>
  </button>
</template>
```

### Task 6.5: Build UecmPageHeader

**Files:**
- Create: `src/components/primitives/UecmPageHeader.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import UecmIcon from "./UecmIcon.vue";

defineProps<{
  title: string;
  subtitle?: string;
  crumbs?: string[];
}>();
</script>

<template>
  <div class="flex flex-col gap-2 border-b border-border bg-card/30 px-6 py-4">
    <div
      v-if="crumbs && crumbs.length"
      class="flex items-center gap-1 font-mono text-[11px] uppercase tracking-wider text-muted-foreground"
    >
      <template v-for="(c, i) in crumbs" :key="i">
        <UecmIcon v-if="i > 0" name="chevron-right" :size="12" />
        <span>{{ c }}</span>
      </template>
    </div>
    <div class="flex items-end justify-between gap-4">
      <div>
        <h1 class="text-xl font-semibold tracking-tight">{{ title }}</h1>
        <p
          v-if="subtitle"
          class="mt-1 text-sm text-muted-foreground"
        >
          {{ subtitle }}
        </p>
      </div>
      <div v-if="$slots.actions" class="flex items-center gap-2">
        <slot name="actions" />
      </div>
    </div>
  </div>
</template>
```

### Task 6.6: Build UecmFilterChip

**Files:**
- Create: `src/components/primitives/UecmFilterChip.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import UecmIcon from "./UecmIcon.vue";

defineProps<{ label: string; value: string }>();
</script>

<template>
  <button
    type="button"
    class="flex h-7 items-center gap-1.5 rounded-md border border-border bg-background px-2 font-mono text-[11px] text-muted-foreground hover:text-foreground"
  >
    <span class="uppercase">{{ label }}:</span>
    <span class="text-foreground">{{ value }}</span>
    <UecmIcon name="chevron-down" :size="12" />
  </button>
</template>
```

### Task 6.7: Build UecmStat

**Files:**
- Create: `src/components/primitives/UecmStat.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import UecmIcon from "./UecmIcon.vue";

type Tone = "healthy" | "warning" | "critical" | "info";

const props = defineProps<{
  icon: string;
  label: string;
  value: number | string;
  tone: Tone;
  href?: string;
}>();

const TONE_CLS: Record<Tone, string> = {
  healthy:
    "text-status-healthy border-[color-mix(in_oklch,var(--status-healthy)_40%,transparent)] bg-[color-mix(in_oklch,var(--status-healthy)_10%,transparent)]",
  warning:
    "text-status-warning border-[color-mix(in_oklch,var(--status-warning)_40%,transparent)] bg-[color-mix(in_oklch,var(--status-warning)_10%,transparent)]",
  critical:
    "text-status-critical border-[color-mix(in_oklch,var(--status-critical)_40%,transparent)] bg-[color-mix(in_oklch,var(--status-critical)_10%,transparent)]",
  info: "text-status-info border-[color-mix(in_oklch,var(--status-info)_40%,transparent)] bg-[color-mix(in_oklch,var(--status-info)_10%,transparent)]",
};
</script>

<template>
  <component
    :is="href ? 'router-link' : 'div'"
    :to="href"
    :class="[
      'flex flex-col gap-1 rounded-md border p-3 transition-colors',
      TONE_CLS[tone],
      href && 'hover:brightness-110',
    ]"
  >
    <div class="flex items-center gap-1.5">
      <UecmIcon :name="icon" :size="14" />
      <span class="font-mono text-[10px] uppercase tracking-wider opacity-90">
        {{ label }}
      </span>
    </div>
    <span class="font-mono text-2xl font-semibold tabular-nums leading-none text-foreground">
      {{ value }}
    </span>
  </component>
</template>
```

### Task 6.8: Build UecmKpiTile + UecmScoreTile

**Files:**
- Create: `src/components/primitives/UecmKpiTile.vue`
- Create: `src/components/primitives/UecmScoreTile.vue`

- [ ] **Step 1: UecmKpiTile.vue**

```vue
<script setup lang="ts">
import UecmIcon from "./UecmIcon.vue";

type Tone = "healthy" | "warning" | "critical" | "offline";

const props = defineProps<{
  label: string;
  value: number | string;
  total: number;
  tone: Tone;
  icon: string;
}>();

const TONE_FG: Record<Tone, string> = {
  healthy: "text-status-healthy",
  warning: "text-status-warning",
  critical: "text-status-critical",
  offline: "text-status-offline",
};
</script>

<template>
  <div class="bg-background px-4 py-3">
    <div class="flex items-center justify-between">
      <div class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
        {{ label }}
      </div>
      <UecmIcon :name="icon" :size="14" :class="TONE_FG[tone]" />
    </div>
    <div class="mt-1 flex items-baseline gap-1">
      <span :class="['font-mono text-2xl font-semibold tabular-nums', TONE_FG[tone]]">
        {{ value }}
      </span>
      <span class="font-mono text-xs text-muted-foreground">/{{ total }}</span>
    </div>
  </div>
</template>
```

- [ ] **Step 2: UecmScoreTile.vue**

```vue
<script setup lang="ts">
type Tone = "healthy" | "warning" | "critical";

const props = defineProps<{
  label: string;
  score: number;
  tone: Tone;
  sub: string;
}>();

const TONE_FG: Record<Tone, string> = {
  healthy: "text-status-healthy",
  warning: "text-status-warning",
  critical: "text-status-critical",
};
</script>

<template>
  <div class="bg-background px-4 py-3">
    <div class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
      {{ label }}
    </div>
    <div class="mt-1 flex items-baseline gap-1">
      <span :class="['font-mono text-2xl font-semibold tabular-nums', TONE_FG[tone]]">
        {{ score }}
      </span>
      <span class="font-mono text-xs text-muted-foreground">/100</span>
    </div>
    <div :class="['mt-0.5 font-mono text-[10px] uppercase', TONE_FG[tone]]">{{ sub }}</div>
  </div>
</template>
```

### Task 6.9: Build UecmAlertRow

**Files:**
- Create: `src/components/primitives/UecmAlertRow.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import UecmIcon from "./UecmIcon.vue";

type Tone = "warning" | "critical";

const props = defineProps<{
  tone: Tone;
  title: string;
  desc: string;
  href?: string;
}>();

const TONE_FG = { warning: "text-status-warning", critical: "text-status-critical" };
const TONE_ICON = { warning: "alert-triangle", critical: "x-octagon" };
</script>

<template>
  <component
    :is="href ? 'router-link' : 'div'"
    :to="href"
    class="flex items-start gap-3 px-4 py-3 hover:bg-accent/30"
  >
    <UecmIcon :name="TONE_ICON[tone]" :size="16" :class="['mt-0.5 shrink-0', TONE_FG[tone]]" />
    <div class="min-w-0 flex-1">
      <div class="text-[12px] font-medium">{{ title }}</div>
      <div class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground">{{ desc }}</div>
    </div>
    <UecmIcon name="chevron-right" :size="14" class="mt-0.5 text-muted-foreground" />
  </component>
</template>
```

### Task 6.10: Build UecmArtifactPill

**Files:**
- Create: `src/components/primitives/UecmArtifactPill.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import UecmIcon from "./UecmIcon.vue";

type Status = "healthy" | "warning" | "critical" | "na";

const props = defineProps<{
  icon: string;
  label: string;
  status: Status;
  detail: string;
}>();

const STATUS_CLS: Record<Status, string> = {
  healthy:
    "border-[color-mix(in_oklch,var(--status-healthy)_40%,transparent)] text-status-healthy bg-[color-mix(in_oklch,var(--status-healthy)_8%,transparent)]",
  warning:
    "border-[color-mix(in_oklch,var(--status-warning)_40%,transparent)] text-status-warning bg-[color-mix(in_oklch,var(--status-warning)_8%,transparent)]",
  critical:
    "border-[color-mix(in_oklch,var(--status-critical)_40%,transparent)] text-status-critical bg-[color-mix(in_oklch,var(--status-critical)_8%,transparent)]",
  na: "border-border text-muted-foreground bg-muted/40",
};
</script>

<template>
  <span
    :class="[
      'inline-flex h-5 items-center gap-1 rounded border px-1.5 font-mono text-[10px]',
      STATUS_CLS[status],
    ]"
  >
    <UecmIcon :name="icon" :size="12" />
    <span class="font-semibold uppercase">{{ label }}</span>
    <span class="opacity-80">· {{ detail }}</span>
  </span>
</template>
```

### Task 6.11: Build UecmCopyText

**Files:**
- Create: `src/components/primitives/UecmCopyText.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import { ref } from "vue";
import UecmIcon from "./UecmIcon.vue";

const props = defineProps<{ text?: string }>();
const copied = ref(false);

async function copy() {
  const value = props.text ?? (event?.currentTarget as HTMLElement)?.textContent ?? "";
  if (!value) return;
  await navigator.clipboard.writeText(value);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1200);
}
</script>

<template>
  <span class="group inline-flex items-center gap-1.5">
    <span class="truncate font-mono text-[11px]">
      <slot>{{ text }}</slot>
    </span>
    <button
      type="button"
      aria-label="Copy"
      class="opacity-0 transition-opacity group-hover:opacity-100"
      @click="copy"
    >
      <UecmIcon
        :name="copied ? 'check' : 'copy'"
        :size="12"
        class="text-muted-foreground hover:text-foreground"
      />
    </button>
  </span>
</template>
```

### Task 6.12: Update UecmKV (uppercase mono key)

**Files:**
- Modify: `src/components/primitives/UecmKV.vue` (full replace)

- [ ] **Step 1: Replace contents**

```vue
<script setup lang="ts">
defineProps<{ k: string }>();
</script>

<template>
  <div class="grid grid-cols-[120px_1fr] items-center gap-2 text-[12px]">
    <span class="font-mono text-[11px] uppercase tracking-wide text-muted-foreground">
      {{ k }}
    </span>
    <span class="min-w-0 truncate text-foreground">
      <slot />
    </span>
  </div>
</template>
```

### Task 6.13: Build UecmDetailCard + UecmSectionHeader

**Files:**
- Create: `src/components/primitives/UecmDetailCard.vue`
- Create: `src/components/primitives/UecmSectionHeader.vue`

- [ ] **Step 1: UecmDetailCard.vue**

```vue
<script setup lang="ts">
defineProps<{ title: string }>();
</script>

<template>
  <div class="rounded-lg border border-border bg-card">
    <div class="border-b border-border px-3 py-2 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
      {{ title }}
    </div>
    <div class="flex flex-col gap-1.5 p-3 text-[12px]">
      <slot />
    </div>
  </div>
</template>
```

- [ ] **Step 2: UecmSectionHeader.vue**

```vue
<script setup lang="ts">
defineProps<{ title: string; count?: number }>();
</script>

<template>
  <div class="flex items-center gap-2">
    <h2 class="text-[13px] font-semibold">{{ title }}</h2>
    <span
      v-if="count !== undefined"
      class="rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
    >
      {{ count }}
    </span>
  </div>
</template>
```

### Task 6.14: Build UecmCodeBlock (INI diff)

**Files:**
- Create: `src/components/primitives/UecmCodeBlock.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import { computed } from "vue";
import UecmIcon from "./UecmIcon.vue";

type Tone = "critical" | "healthy";

const props = defineProps<{
  title: string;
  tone: Tone;
  code: string;
  startLine: number;
  highlightLine?: number;
}>();

const TONE_BORDER = {
  critical: "border-[color-mix(in_oklch,var(--status-critical)_40%,transparent)]",
  healthy: "border-[color-mix(in_oklch,var(--status-healthy)_40%,transparent)]",
};
const TONE_HEADER = {
  critical: "text-status-critical bg-[color-mix(in_oklch,var(--status-critical)_8%,transparent)]",
  healthy: "text-status-healthy bg-[color-mix(in_oklch,var(--status-healthy)_8%,transparent)]",
};
const TONE_LINE_HIGHLIGHT = {
  critical: "bg-[color-mix(in_oklch,var(--status-critical)_18%,transparent)]",
  healthy: "bg-[color-mix(in_oklch,var(--status-healthy)_15%,transparent)]",
};

const lines = computed(() => props.code.split("\n"));
</script>

<template>
  <div :class="['overflow-hidden rounded-md border bg-background', TONE_BORDER[tone]]">
    <div
      :class="[
        'flex h-8 items-center justify-between border-b border-border px-3 font-mono text-[11px] uppercase tracking-wide',
        TONE_HEADER[tone],
      ]"
    >
      <span class="font-semibold">{{ title }}</span>
      <button class="flex items-center gap-1 text-muted-foreground hover:text-foreground">
        <UecmIcon name="copy" :size="12" />
      </button>
    </div>
    <pre class="overflow-x-auto px-0 py-2 font-mono text-[11px] leading-5">
      <div
        v-for="(line, i) in lines"
        :key="i"
        :class="[
          'flex gap-3 px-3',
          highlightLine !== undefined && i === highlightLine - startLine && TONE_LINE_HIGHLIGHT[tone],
        ]"
      ><span class="w-8 shrink-0 select-none text-right text-muted-foreground">{{ startLine + i }}</span><span class="text-foreground/90">{{ line || ' ' }}</span></div>
    </pre>
  </div>
</template>
```

### Task 6.15: Build UecmRoleTag (Host/Render Node/Editor/Dev)

**Files:**
- Create: `src/components/primitives/UecmRoleTag.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import type { MachineRole } from "@/lib/mockData";

defineProps<{ role: MachineRole }>();

const ROLE_CLS: Record<MachineRole, string> = {
  Host: "border-primary/40 text-primary bg-primary/10",
  "Render Node": "border-border bg-muted text-foreground",
  Editor:
    "border-[color-mix(in_oklch,var(--status-info)_40%,transparent)] text-status-info bg-[color-mix(in_oklch,var(--status-info)_10%,transparent)]",
  Dev: "border-border bg-muted/60 text-muted-foreground",
};
</script>

<template>
  <span
    :class="[
      'inline-flex h-4 items-center rounded border px-1 font-mono text-[10px] uppercase tracking-wide',
      ROLE_CLS[role],
    ]"
  >
    {{ role }}
  </span>
</template>
```

### Task 6.16: Update primitives barrel + build verification

**Files:**
- Modify: `src/components/primitives/index.ts`

- [ ] **Step 1: Replace contents**

```ts
export { default as UecmIcon } from "./UecmIcon.vue";
export { default as UecmStatusDot } from "./UecmStatusDot.vue";
export { default as UecmStatusBadge } from "./UecmStatusBadge.vue";
export { default as UecmMatrixCell } from "./UecmMatrixCell.vue";
export { default as UecmPageHeader } from "./UecmPageHeader.vue";
export { default as UecmFilterChip } from "./UecmFilterChip.vue";
export { default as UecmStat } from "./UecmStat.vue";
export { default as UecmKpiTile } from "./UecmKpiTile.vue";
export { default as UecmScoreTile } from "./UecmScoreTile.vue";
export { default as UecmAlertRow } from "./UecmAlertRow.vue";
export { default as UecmArtifactPill } from "./UecmArtifactPill.vue";
export { default as UecmCopyText } from "./UecmCopyText.vue";
export { default as UecmKV } from "./UecmKV.vue";
export { default as UecmDetailCard } from "./UecmDetailCard.vue";
export { default as UecmSectionHeader } from "./UecmSectionHeader.vue";
export { default as UecmCodeBlock } from "./UecmCodeBlock.vue";
export { default as UecmRoleTag } from "./UecmRoleTag.vue";
export { default as UecmThemeToggle } from "./UecmThemeToggle.vue";
export type { Tone } from "./types";
```

- [ ] **Step 2: Build + test**

```bash
pnpm build 2>&1 | tail -5
pnpm test 2>&1 | tail -8
```

Expected: build passes; tests pass (some legacy tests may be broken from earlier deletions — that's OK if they're for deleted components).

- [ ] **Step 3: Commit**

```bash
git add src/components/primitives
git commit -m "feat(primitives): rebuild StatusBadge/MatrixCell/PageHeader/Stat/KpiTile/AlertRow/ArtifactPill/CopyText/KV/DetailCard/CodeBlock/RoleTag for new design"
```

---

## Phase 7 — Global Shell Rebuild (90 min)

### Task 7.1: Build UecmSidebar (240px, with cluster mini + Issues)

**Files:**
- Create: `src/components/shell/UecmSidebar.vue`
- Delete: `src/components/shell/ActivityBar.vue`

- [ ] **Step 1: Write UecmSidebar.vue**

```vue
<script setup lang="ts">
import { RouterLink } from "vue-router";
import { useClusterStore } from "@/stores/cluster";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmStatusDot from "@/components/primitives/UecmStatusDot.vue";

interface NavItem {
  to: string;
  label: string;
  icon: string;
  badge?: string;
  badgeTone?: "muted" | "critical";
}

const cluster = useClusterStore();

const NAV: NavItem[] = [
  { to: "/", label: "Dashboard", icon: "layout-dashboard" },
  { to: "/machines", label: "Machines", icon: "server", badge: "10", badgeTone: "muted" },
  { to: "/projects", label: "Projects", icon: "folder-git-2", badge: "3", badgeTone: "muted" },
  { to: "/ddc-pak", label: "DDC Pak", icon: "package" },
  { to: "/pso-cache", label: "PSO Cache", icon: "cpu" },
  { to: "/ini-scanner", label: "INI Scanner", icon: "file-search", badge: "5", badgeTone: "critical" },
  { to: "/health-check", label: "Health Check", icon: "activity", badge: "!", badgeTone: "critical" },
];
</script>

<template>
  <aside class="flex h-full w-[240px] shrink-0 flex-col border-r border-border bg-sidebar text-sidebar-foreground">
    <div class="flex h-12 items-center gap-2 border-b border-sidebar-border px-3">
      <div class="flex h-7 w-7 items-center justify-center rounded-md bg-primary/15 text-primary">
        <UecmIcon name="circuit-board" :size="16" />
      </div>
      <div class="flex flex-col leading-tight">
        <span class="font-mono text-[13px] font-semibold tracking-wide">UECM</span>
        <span class="font-mono text-[10px] text-muted-foreground">{{ cluster.version }} · {{ cluster.name }}</span>
      </div>
    </div>

    <div class="border-b border-sidebar-border px-3 py-3">
      <div class="flex items-center justify-between">
        <span class="text-[11px] uppercase tracking-wider text-muted-foreground">Cluster</span>
        <span class="font-mono text-[11px] text-status-warning">{{ cluster.statusLabel }}</span>
      </div>
      <div class="mt-2 flex items-end gap-1.5">
        <span class="font-mono text-2xl font-semibold tabular-nums">{{ cluster.score }}</span>
        <span class="mb-1 text-xs text-muted-foreground">/100</span>
        <span class="mb-1 ml-auto font-mono text-[11px] text-muted-foreground">{{ cluster.online }}/{{ cluster.total }} online</span>
      </div>
      <div class="mt-2 h-1 w-full overflow-hidden rounded-full bg-muted">
        <div class="h-full rounded-full bg-status-warning" :style="{ width: `${cluster.score}%` }" />
      </div>
    </div>

    <nav class="flex-1 overflow-y-auto py-2">
      <div class="px-3 pb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Workspace
      </div>
      <ul class="space-y-0.5 px-2">
        <li v-for="item in NAV" :key="item.to">
          <RouterLink
            :to="item.to"
            data-nav-item
            v-slot="{ isActive }"
            custom
          >
            <a
              :class="[
                'group flex h-8 items-center gap-2 rounded-md px-2 text-[13px] transition-colors hover:bg-sidebar-accent',
                isActive && 'bg-sidebar-accent text-sidebar-accent-foreground',
              ]"
              :href="item.to"
              data-nav-item
              @click.prevent="$router.push(item.to)"
            >
              <UecmIcon
                :name="item.icon"
                :size="16"
                :class="[
                  'text-muted-foreground group-hover:text-foreground',
                  isActive && '!text-primary',
                ]"
              />
              <span :class="[isActive && 'font-medium']">{{ item.label }}</span>
              <span
                v-if="item.badge"
                :class="[
                  'ml-auto inline-flex h-4 min-w-4 items-center justify-center rounded border px-1 font-mono text-[10px]',
                  item.badgeTone === 'critical'
                    ? 'text-status-critical bg-[color-mix(in_oklch,var(--status-critical)_15%,transparent)] border-[color-mix(in_oklch,var(--status-critical)_40%,transparent)]'
                    : 'text-muted-foreground bg-muted border-border',
                ]"
              >
                {{ item.badge }}
              </span>
            </a>
          </RouterLink>
        </li>
      </ul>

      <div class="mt-4 px-3 pb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Issues
      </div>
      <div class="space-y-1 px-2">
        <RouterLink
          to="/health-check"
          class="flex h-8 items-center gap-2 rounded-md px-2 text-[12px] hover:bg-sidebar-accent"
        >
          <UecmStatusDot status="critical" :size="6" />
          <span>Critical</span>
          <span class="ml-auto font-mono text-muted-foreground">{{ cluster.criticalCount }}</span>
        </RouterLink>
        <RouterLink
          to="/ini-scanner"
          class="flex h-8 items-center gap-2 rounded-md px-2 text-[12px] hover:bg-sidebar-accent"
        >
          <UecmStatusDot status="warning" :size="6" />
          <span>Warning</span>
          <span class="ml-auto font-mono text-muted-foreground">{{ cluster.warningCount }}</span>
        </RouterLink>
        <div class="flex h-8 items-center gap-2 rounded-md px-2 text-[12px] text-muted-foreground">
          <UecmStatusDot status="offline" :size="6" />
          <span>Offline</span>
          <span class="ml-auto font-mono">{{ cluster.offlineCount }}</span>
        </div>
      </div>
    </nav>

    <div class="border-t border-sidebar-border p-2">
      <button class="flex h-8 w-full items-center gap-2 rounded-md px-2 text-[12px] text-muted-foreground hover:bg-sidebar-accent">
        <UecmIcon name="settings" :size="16" />
        Settings
      </button>
      <button class="flex h-8 w-full items-center gap-2 rounded-md px-2 text-[12px] text-muted-foreground hover:bg-sidebar-accent">
        <UecmIcon name="help-circle" :size="16" />
        Documentation
      </button>
    </div>
  </aside>
</template>
```

- [ ] **Step 2: Delete ActivityBar.vue**

```bash
rm src/components/shell/ActivityBar.vue
```

### Task 7.2: Build UecmTopBar (with global active-task pill)

**Files:**
- Create: `src/components/shell/UecmTopBar.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import { Input } from "@/components/ui/Input.vue";
import { Button } from "@/components/ui/Button.vue";
import { useTasksStore } from "@/stores/tasks";
import { useClusterStore } from "@/stores/cluster";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmStatusDot from "@/components/primitives/UecmStatusDot.vue";
import UecmThemeToggle from "@/components/primitives/UecmThemeToggle.vue";

const tasks = useTasksStore();
const cluster = useClusterStore();

defineProps<{
  logOpen: boolean;
}>();

const emit = defineEmits<{ "toggle-log": [] }>();
</script>

<template>
  <header class="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card/40 px-3">
    <button class="flex h-8 items-center gap-2 rounded-md border border-border bg-background px-2.5 text-[12px] hover:bg-accent">
      <UecmStatusDot status="healthy" :size="6" />
      <span class="font-mono">HOST-01</span>
      <span class="text-muted-foreground">·</span>
      <span class="text-muted-foreground">192.168.10.10</span>
      <UecmIcon name="chevron-down" :size="14" class="ml-1 text-muted-foreground" />
    </button>

    <div class="relative ml-1 max-w-xl flex-1">
      <UecmIcon name="search" :size="14" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
      <Input
        placeholder="Search machines, projects, ini keys, paths…  (⌘K)"
        class="h-8 border-border bg-background pl-8 pr-16 font-mono text-[12px] placeholder:text-muted-foreground/70"
      />
      <kbd class="pointer-events-none absolute right-2 top-1/2 inline-flex h-5 -translate-y-1/2 items-center gap-1 rounded border border-border bg-muted/60 px-1.5 font-mono text-[10px] text-muted-foreground">
        ⌘K
      </kbd>
    </div>

    <div class="flex-1" />

    <div
      v-if="tasks.activeTask"
      class="flex h-8 items-center gap-2 rounded-md border border-[color-mix(in_oklch,var(--status-info)_40%,transparent)] bg-[color-mix(in_oklch,var(--status-info)_10%,transparent)] px-2.5 text-[12px]"
    >
      <UecmIcon name="loader-2" :size="14" class="animate-spin text-status-info" />
      <span class="font-mono uppercase tracking-wide text-status-info">
        {{ tasks.activeTask.type }}
      </span>
      <span class="text-muted-foreground">·</span>
      <span class="font-mono">{{ tasks.activeTask.project ?? tasks.activeTask.targets?.[0] }}</span>
      <div class="h-1 w-16 overflow-hidden rounded-full bg-muted">
        <div class="h-full bg-status-info" :style="{ width: `${tasks.activeTask.progress}%` }" />
      </div>
      <span class="font-mono tabular-nums text-muted-foreground">{{ tasks.activeTask.progress }}%</span>
      <button
        class="ml-1 text-muted-foreground hover:text-foreground"
        aria-label="Pause task"
        @click="tasks.pauseTask(tasks.activeTask.id)"
      >
        <UecmIcon name="pause" :size="14" />
      </button>
      <button
        class="text-muted-foreground hover:text-foreground"
        aria-label="Cancel task"
        @click="tasks.cancelTask(tasks.activeTask.id)"
      >
        <UecmIcon name="x" :size="14" />
      </button>
    </div>

    <div class="hidden items-center gap-3 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] md:flex">
      <span class="flex items-center gap-1.5">
        <UecmStatusDot status="healthy" :size="6" />
        <span class="font-mono">{{ cluster.healthyCount }}</span>
      </span>
      <span class="flex items-center gap-1.5">
        <UecmStatusDot status="warning" :size="6" />
        <span class="font-mono">{{ cluster.warningCount }}</span>
      </span>
      <span class="flex items-center gap-1.5">
        <UecmStatusDot status="critical" :size="6" />
        <span class="font-mono">{{ cluster.criticalCount }}</span>
      </span>
      <span class="text-muted-foreground">·</span>
      <span class="font-mono text-muted-foreground">{{ cluster.online }}/{{ cluster.total }} online</span>
    </div>

    <UecmThemeToggle />

    <button
      class="relative inline-flex h-8 w-8 items-center justify-center rounded-md hover:bg-accent text-foreground"
      aria-label="Notifications"
    >
      <UecmIcon name="alert-triangle" :size="16" />
      <span class="absolute right-1.5 top-1.5 h-1.5 w-1.5 rounded-full bg-status-critical" />
    </button>

    <Button
      variant="outline"
      size="sm"
      class="h-8 gap-1.5"
      :aria-pressed="logOpen"
      @click="emit('toggle-log')"
    >
      <UecmIcon name="terminal" :size="14" />
      <span class="font-mono text-[11px] uppercase">Logs</span>
    </Button>
  </header>
</template>
```

### Task 7.3: Build UecmLogPanel

**Files:**
- Create: `src/components/shell/UecmLogPanel.vue`

- [ ] **Step 1: Write component**

```vue
<script setup lang="ts">
import { computed } from "vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import { LOG_LINES } from "@/lib/mockData";

defineEmits<{ close: [] }>();

const lines = computed(() => LOG_LINES);
</script>

<template>
  <section class="flex h-[200px] shrink-0 flex-col border-t border-border bg-card/60">
    <div class="flex h-9 items-center gap-2 border-b border-border px-3">
      <span class="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">
        Realtime Log
      </span>
      <span class="ml-2 inline-flex items-center gap-1 rounded border border-border bg-muted/40 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
        <span class="h-1.5 w-1.5 animate-pulse rounded-full bg-status-info" />
        streaming
      </span>
      <div class="ml-3 flex items-center gap-1">
        <button class="flex h-6 items-center gap-1 rounded border border-border px-1.5 font-mono text-[10px] text-muted-foreground hover:text-foreground">
          <UecmIcon name="filter" :size="12" /> ALL
        </button>
        <button class="flex h-6 items-center gap-1 rounded border border-border px-1.5 font-mono text-[10px] text-muted-foreground hover:text-foreground">
          INFO
        </button>
        <button class="flex h-6 items-center gap-1 rounded border border-border px-1.5 font-mono text-[10px] text-status-warning hover:text-foreground">
          WARN
        </button>
        <button class="flex h-6 items-center gap-1 rounded border border-border px-1.5 font-mono text-[10px] text-status-critical hover:text-foreground">
          ERR
        </button>
      </div>
      <div class="ml-auto flex items-center gap-1">
        <button class="flex h-6 items-center gap-1 rounded border border-border px-1.5 text-muted-foreground hover:text-foreground">
          <UecmIcon name="pause" :size="12" />
        </button>
        <button class="flex h-6 items-center gap-1 rounded border border-border px-1.5 text-muted-foreground hover:text-foreground">
          <UecmIcon name="copy" :size="12" />
        </button>
        <button class="flex h-6 items-center gap-1 rounded border border-border px-1.5 text-muted-foreground hover:text-foreground">
          <UecmIcon name="trash-2" :size="12" />
        </button>
        <button
          class="flex h-6 items-center gap-1 rounded border border-border px-1.5 font-mono text-[10px] text-muted-foreground hover:text-foreground"
          @click="$emit('close')"
        >
          <UecmIcon name="chevron-down" :size="12" /> Hide
        </button>
      </div>
    </div>
    <div class="flex-1 overflow-y-auto px-3 py-1 font-mono text-[11px] leading-5">
      <div
        v-for="(l, i) in lines"
        :key="i"
        class="flex gap-3 hover:bg-accent/30"
      >
        <span class="w-24 shrink-0 text-muted-foreground">{{ l.t }}</span>
        <span
          :class="[
            'w-12 shrink-0',
            l.lvl.trim() === 'INFO' && 'text-muted-foreground',
            l.lvl.trim() === 'WARN' && 'text-status-warning',
            l.lvl.trim() === 'ERR' && 'text-status-critical',
          ]"
        >{{ l.lvl }}</span>
        <span class="w-32 shrink-0 text-primary">{{ l.src }}</span>
        <span class="text-foreground/90">{{ l.msg }}</span>
      </div>
    </div>
  </section>
</template>
```

### Task 7.4: Rewrite AppShell (sidebar + topbar + main + log panel)

**Files:**
- Modify: `src/components/shell/AppShell.vue` (full replace)

- [ ] **Step 1: Replace contents**

```vue
<script setup lang="ts">
import { ref } from "vue";
import { RouterView } from "vue-router";
import UecmSidebar from "./UecmSidebar.vue";
import UecmTopBar from "./UecmTopBar.vue";
import UecmLogPanel from "./UecmLogPanel.vue";

const logOpen = ref(false);
</script>

<template>
  <div class="flex h-full w-full overflow-hidden bg-background text-foreground">
    <UecmSidebar />
    <div class="flex h-full flex-1 flex-col overflow-hidden">
      <UecmTopBar :log-open="logOpen" @toggle-log="logOpen = !logOpen" />
      <main class="flex-1 overflow-y-auto">
        <RouterView />
      </main>
      <UecmLogPanel v-if="logOpen" @close="logOpen = false" />
    </div>
  </div>
</template>
```

### Task 7.5: Update AppShell test (8 nav items → 7)

**Files:**
- Modify: `src/__tests__/AppShell.spec.ts`

- [ ] **Step 1: Read current test**

```bash
cat src/__tests__/AppShell.spec.ts
```

- [ ] **Step 2: Update expected count**

Change line 26:
```ts
expect(navItems).toHaveLength(8);
```
to:
```ts
expect(navItems).toHaveLength(7);
```

- [ ] **Step 3: Run, verify pass**

```bash
pnpm vitest run src/__tests__/AppShell.spec.ts
```

Expected: 2 PASS.

### Task 7.6: Remove `/shares` from router (kept as a destination, not nav-accessible)

**Files:**
- Modify: `src/router/index.ts`

- [ ] **Step 1: Read current router**

Inspect `src/router/index.ts` to confirm `/shares` route exists.

- [ ] **Step 2: Verify route exists**

Per design decision A: Shares stays as a route (so `ShareCreateWizard.vue` and `Shares.vue` remain accessible). It just isn't in the sidebar nav. **No changes to router needed.**

- [ ] **Step 3: Verify all tests still pass**

```bash
pnpm test 2>&1 | tail -10
```

Expected: all tests pass; AppShell now expects 7 nav items.

- [ ] **Step 4: Commit**

```bash
git add src/components/shell src/__tests__/AppShell.spec.ts
git commit -m "feat(shell): rebuild AppShell with 240px sidebar + topbar + log panel + active-task pill"
```

---

## Phase 8 — Translate Views (3.5–4 hrs)

For each view: rewrite the template per the Next.js source, port mock data references, **preserve all `data-*` selectors used by tests**, run tests after each.

> **Translation contract for views 8.2–8.7:** The React sources at `/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/<route>/page.tsx` are the **canonical visual blueprint**. Each task below specifies (a) the source file path, (b) the exact primitives + shadcn components to compose with, (c) the stores to wire, (d) the `data-*` selectors that MUST remain on the rewritten template for existing tests to pass. The engineer should keep two windows side-by-side — React source on the left, Vue file on the right — and translate component by component. Use the full Dashboard code in Task 8.1 as the canonical Vue translation pattern (script setup with TS, RouterLink, mockData imports, primitives barrel). Don't paraphrase or shorten the React mockup — every section, every callout, every conditional class is intentional.

### Task 8.1: Translate Dashboard.vue

**Files:**
- Modify: `src/views/Dashboard.vue` (full replace)

- [ ] **Step 1: Read source design**

Reference: `/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/page.tsx` (already read; 434 lines).

- [ ] **Step 2: Write Vue equivalent**

Use the React source as the visual blueprint. Compose with: `UecmPageHeader`, `UecmStat`, `UecmStatusBadge`, `UecmAlertRow`, `Button` (shadcn). Wire `tasks` and `cluster` stores.

Note: The mock data fields `CLUSTER_HEALTH_PCT`, `HEALTHY_COUNT`, `WARNING_COUNT`, `CRITICAL_COUNT`, `RECENT_OPS`, `RUNNING_TASKS` come from `@/lib/mockData`.

The `data-bridge-test-btn` selector test (Dashboard-view.spec.ts) must still find a clickable button. Map it to a "Run check now" button OR add the bridge test as a small Dev Tools section at the bottom, hidden behind a collapsible details. Acceptable compromise: add a small footer section inside Dashboard that contains the bridge button with `data-bridge-test-btn` attribute (visually subtle but keeps tests green).

Full file content:

```vue
<script setup lang="ts">
import { ref } from "vue";
import { RouterLink } from "vue-router";
import { tauriApi, type EchoResult, type UecmError } from "@/services/tauri";
import { useTasksStore } from "@/stores/tasks";
import { useClusterStore } from "@/stores/cluster";
import {
  RECENT_OPS,
  CLUSTER_HEALTH_PCT,
  HEALTHY_COUNT,
  WARNING_COUNT,
  CRITICAL_COUNT,
  ONLINE,
  TOTAL,
} from "@/lib/mockData";
import { Button } from "@/components/ui/Button.vue";
import {
  UecmPageHeader,
  UecmStat,
  UecmStatusBadge,
  UecmAlertRow,
  UecmIcon,
} from "@/components/primitives";

const tasks = useTasksStore();
const cluster = useClusterStore();

const result = ref<EchoResult | null>(null);
const error = ref<UecmError | null>(null);
const loading = ref(false);

async function runBridgeTest() {
  result.value = null;
  error.value = null;
  loading.value = true;
  try {
    result.value = await tauriApi.testPowerShellBridge("hello from UECM");
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="flex flex-col">
    <UecmPageHeader
      title="Cluster Overview"
      :subtitle="`${cluster.name} · ${TOTAL} machines · last health check 14 min ago`"
      :crumbs="['UECM', 'Dashboard']"
    >
      <template #actions>
        <Button variant="outline" size="sm" class="h-8 gap-1.5">
          <UecmIcon name="refresh-cw" :size="14" />
          <span class="text-[12px]">Run check now</span>
        </Button>
        <Button size="sm" class="h-8 gap-1.5">
          <UecmIcon name="play-circle" :size="14" />
          <span class="text-[12px]">One-stop wizard</span>
        </Button>
      </template>
    </UecmPageHeader>

    <div class="grid grid-cols-12 gap-4 p-6">
      <section class="col-span-12 lg:col-span-8">
        <div class="rounded-lg border border-border bg-card p-5">
          <div class="flex items-start justify-between">
            <div>
              <div class="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">
                Cluster Health
              </div>
              <div class="mt-2 flex items-end gap-3">
                <span class="font-mono text-5xl font-semibold tabular-nums leading-none">
                  {{ CLUSTER_HEALTH_PCT }}
                </span>
                <span class="mb-1 text-sm text-muted-foreground">/100</span>
                <UecmStatusBadge status="warning" label="DEGRADED" size="sm" class="mb-1.5" />
              </div>
            </div>
            <div class="hidden flex-col items-end gap-1 md:flex">
              <span class="font-mono text-[11px] uppercase text-muted-foreground">Online</span>
              <span class="font-mono text-2xl tabular-nums">
                {{ ONLINE }}<span class="text-muted-foreground">/{{ TOTAL }}</span>
              </span>
            </div>
          </div>

          <div class="mt-4 flex h-2 w-full overflow-hidden rounded-full bg-muted">
            <div class="bg-status-healthy" :style="{ width: `${(HEALTHY_COUNT / TOTAL) * 100}%` }" />
            <div class="bg-status-warning" :style="{ width: `${(WARNING_COUNT / TOTAL) * 100}%` }" />
            <div class="bg-status-critical" :style="{ width: `${(CRITICAL_COUNT / TOTAL) * 100}%` }" />
            <div class="bg-status-offline" :style="{ width: `${((TOTAL - ONLINE) / TOTAL) * 100}%` }" />
          </div>

          <div class="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
            <UecmStat icon="check-circle-2" tone="healthy" label="Healthy" :value="HEALTHY_COUNT" href="/health-check" />
            <UecmStat icon="alert-triangle" tone="warning" label="Warnings" :value="WARNING_COUNT" href="/health-check" />
            <UecmStat icon="x-octagon" tone="critical" label="Critical" :value="CRITICAL_COUNT" href="/ini-scanner" />
            <UecmStat icon="activity" tone="info" label="Active tasks" :value="tasks.running.length" href="/ddc-pak" />
          </div>
        </div>

        <div class="mt-4 rounded-lg border border-border bg-card">
          <div class="flex h-10 items-center justify-between border-b border-border px-4">
            <span class="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">
              Quick Actions
            </span>
            <span class="text-[11px] text-muted-foreground">Most common workflows</span>
          </div>
          <div class="grid grid-cols-2 gap-px bg-border md:grid-cols-5">
            <button class="group flex flex-col items-start gap-1.5 bg-card p-4 text-left transition-colors hover:bg-accent/40">
              <div class="flex h-7 w-7 items-center justify-center rounded-md border border-border bg-background text-primary">
                <UecmIcon name="database" :size="14" />
              </div>
              <div class="text-[13px] font-medium">Create Shared DDC</div>
              <div class="font-mono text-[11px] text-muted-foreground">Host & UNC setup</div>
              <UecmIcon name="chevron-right" :size="14" class="mt-1 text-muted-foreground transition-transform group-hover:translate-x-0.5" />
            </button>
            <button class="group flex flex-col items-start gap-1.5 bg-card p-4 text-left transition-colors hover:bg-accent/40">
              <div class="flex h-7 w-7 items-center justify-center rounded-md border border-border bg-background text-primary">
                <UecmIcon name="send" :size="14" />
              </div>
              <div class="text-[13px] font-medium">Push Config to All</div>
              <div class="font-mono text-[11px] text-muted-foreground">env + ini bundle</div>
              <UecmIcon name="chevron-right" :size="14" class="mt-1 text-muted-foreground" />
            </button>
            <button class="group flex flex-col items-start gap-1.5 bg-card p-4 text-left transition-colors hover:bg-accent/40">
              <div class="flex h-7 w-7 items-center justify-center rounded-md border border-border bg-background text-primary">
                <UecmIcon name="package" :size="14" />
              </div>
              <div class="text-[13px] font-medium">Generate DDC Pak</div>
              <div class="font-mono text-[11px] text-muted-foreground">Source: HOST-01</div>
              <UecmIcon name="chevron-right" :size="14" class="mt-1 text-muted-foreground" />
            </button>
            <button class="group flex flex-col items-start gap-1.5 bg-card p-4 text-left transition-colors hover:bg-accent/40">
              <div class="flex h-7 w-7 items-center justify-center rounded-md border border-border bg-background text-primary">
                <UecmIcon name="cpu" :size="14" />
              </div>
              <div class="text-[13px] font-medium">Collect PSO</div>
              <div class="font-mono text-[11px] text-muted-foreground">EDITOR-01 · EXLY</div>
              <UecmIcon name="chevron-right" :size="14" class="mt-1 text-muted-foreground" />
            </button>
            <button class="group flex flex-col items-start gap-1.5 bg-card p-4 text-left transition-colors hover:bg-accent/40">
              <div class="flex h-7 w-7 items-center justify-center rounded-md border border-border bg-background text-primary">
                <UecmIcon name="activity" :size="14" />
              </div>
              <div class="text-[13px] font-medium">Run Health Check</div>
              <div class="font-mono text-[11px] text-muted-foreground">11 checks · 10 hosts</div>
              <UecmIcon name="chevron-right" :size="14" class="mt-1 text-muted-foreground" />
            </button>
          </div>
        </div>

        <div class="mt-4 rounded-lg border border-border bg-card">
          <div class="flex h-10 items-center justify-between border-b border-border px-4">
            <span class="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">
              Recent Operations
            </span>
            <a class="font-mono text-[11px] uppercase text-muted-foreground hover:text-foreground" href="#">
              View all
            </a>
          </div>
          <table class="w-full text-[12px]">
            <thead>
              <tr class="border-b border-border bg-muted/30 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                <th class="w-16 px-4 py-2 text-left">Time</th>
                <th class="w-40 px-4 py-2 text-left">Action</th>
                <th class="px-4 py-2 text-left">Target</th>
                <th class="w-24 px-4 py-2 text-left">User</th>
                <th class="w-28 px-4 py-2 text-left">Result</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="op in RECENT_OPS" :key="op.id" class="border-b border-border/60 hover:bg-accent/30">
                <td class="px-4 py-2 font-mono tabular-nums text-muted-foreground">{{ op.time }}</td>
                <td class="px-4 py-2 font-medium">{{ op.action }}</td>
                <td class="px-4 py-2 font-mono text-muted-foreground">{{ op.target }}</td>
                <td class="px-4 py-2 font-mono text-muted-foreground">@{{ op.user }}</td>
                <td class="px-4 py-2"><UecmStatusBadge :status="op.result" size="xs" /></td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <aside class="col-span-12 flex flex-col gap-4 lg:col-span-4">
        <div
          v-if="tasks.activeTask"
          class="rounded-lg border border-[color-mix(in_oklch,var(--status-info)_30%,transparent)] bg-[color-mix(in_oklch,var(--status-info)_8%,transparent)] p-4"
        >
          <div class="flex items-center gap-2">
            <UecmIcon name="loader-2" :size="16" class="animate-spin text-status-info" />
            <span class="font-mono text-[11px] uppercase tracking-wider text-status-info">Active Task</span>
            <span class="ml-auto font-mono text-[11px] text-muted-foreground">{{ tasks.activeTask.elapsed }}</span>
          </div>
          <div class="mt-2 text-[13px] font-medium">
            {{ tasks.activeTask.type }} · <span class="font-mono">{{ tasks.activeTask.project }}</span>
          </div>
          <div class="mt-1 font-mono text-[11px] text-muted-foreground">{{ tasks.activeTask.stage }}</div>
          <div class="mt-3 flex items-center gap-2">
            <div class="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
              <div class="h-full bg-status-info" :style="{ width: `${tasks.activeTask.progress}%` }" />
            </div>
            <span class="font-mono text-[11px] tabular-nums">{{ tasks.activeTask.progress }}%</span>
          </div>
          <div class="mt-3 grid grid-cols-3 gap-2 text-[11px]">
            <div>
              <div class="text-muted-foreground">Source</div>
              <div class="font-mono">{{ tasks.activeTask.source }}</div>
            </div>
            <div>
              <div class="text-muted-foreground">Targets</div>
              <div class="font-mono">{{ tasks.activeTask.targets?.length ?? 0 }} nodes</div>
            </div>
            <div>
              <div class="text-muted-foreground">ETA</div>
              <div class="font-mono">{{ tasks.activeTask.remaining }}</div>
            </div>
          </div>
        </div>

        <div class="rounded-lg border border-border bg-card">
          <div class="flex h-10 items-center justify-between border-b border-border px-4">
            <span class="flex items-center gap-2">
              <span class="h-1.5 w-1.5 rounded-full bg-status-critical animate-pulse" aria-hidden="true" />
              <span class="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">Unread Critical</span>
            </span>
            <span class="font-mono text-[11px] tabular-nums text-status-critical">{{ cluster.criticalCount }} new</span>
          </div>
          <div class="divide-y divide-border">
            <UecmAlertRow
              tone="critical"
              title="RENDER-02 — SYSTEM write test failed"
              desc="PsExec -s cmd /c echo OK > \\HOST-01\UE-DDC\.write-test → Access Denied"
              href="/health-check"
            />
            <UecmAlertRow
              tone="critical"
              title="RENDER-02 — DDC override conflict"
              desc="DefaultEngine.ini:142 overrides [DerivedDataBackendGraph]"
              href="/ini-scanner"
            />
            <UecmAlertRow
              tone="warning"
              title="GPU driver mismatch"
              desc="RENDER-02 on 537.13 (cluster: 545.92 / 551.86)"
              href="/pso-cache"
            />
            <UecmAlertRow
              tone="warning"
              title="3 ini findings need rescan"
              desc="Last fix applied 17 min ago"
              href="/ini-scanner"
            />
          </div>
        </div>

        <div class="rounded-lg border border-border bg-card p-4">
          <div class="flex items-center gap-2">
            <UecmIcon name="clock" :size="14" class="text-muted-foreground" />
            <span class="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">Last Health Check</span>
          </div>
          <div class="mt-2 font-mono text-[13px]">2026-05-02 13:55:08</div>
          <div class="mt-1 text-[11px] text-muted-foreground">11 checks · 10 hosts · 2m 31s</div>
          <div class="mt-3 flex items-center gap-2">
            <Button variant="outline" size="sm" class="h-7 flex-1 gap-1.5">
              <UecmIcon name="refresh-cw" :size="12" />
              <span class="text-[11px]">Recheck all</span>
            </Button>
            <Button variant="ghost" size="sm" class="h-7 gap-1.5">
              <span class="text-[11px]">Schedule</span>
            </Button>
          </div>
        </div>

        <div class="rounded-lg border border-dashed border-border bg-card/50 p-4">
          <div class="flex items-center gap-2">
            <UecmIcon name="server" :size="14" class="text-muted-foreground" />
            <span class="font-mono text-[11px] uppercase tracking-wider text-muted-foreground">Tip · Empty state</span>
          </div>
          <p class="mt-2 text-[12px] text-muted-foreground">
            First launch with no machines? Go to
            <RouterLink to="/machines" class="text-primary hover:underline">Machines → Scan LAN</RouterLink>
            or add a machine manually by IP.
          </p>
          <Button variant="outline" size="sm" class="mt-3 h-7 gap-1.5">
            <UecmIcon name="plus" :size="12" />
            <span class="text-[11px]">Add machine by IP</span>
          </Button>
        </div>

        <details class="rounded-lg border border-border bg-card p-4 text-[11px]">
          <summary class="cursor-pointer font-mono uppercase tracking-wider text-muted-foreground">
            Dev tools — PowerShell bridge smoke test
          </summary>
          <p class="mt-2 text-muted-foreground">
            Verifies frontend → Rust → PowerShell sidecar pipeline works.
            On non-Windows dev machines, this will return a "Windows-only" error — that's expected.
          </p>
          <button
            data-bridge-test-btn
            :disabled="loading"
            class="mt-2 px-3 py-1 bg-muted rounded text-[11px] hover:bg-accent disabled:opacity-50"
            @click="runBridgeTest"
          >
            {{ loading ? "Running..." : "Run bridge test" }}
          </button>
          <pre v-if="result" class="mt-3 p-3 bg-muted/30 border border-border rounded text-[10px]">{{ JSON.stringify(result, null, 2) }}</pre>
          <p v-if="error" class="mt-3 text-[11px] text-status-critical">{{ error.code }}: {{ error.message }}</p>
        </details>
      </aside>
    </div>
  </div>
</template>
```

- [ ] **Step 3: Run dashboard tests**

```bash
pnpm vitest run src/__tests__/Dashboard-view.spec.ts
```

Expected: 2 PASS (the test looks for `data-bridge-test-btn` which is preserved in the `<details>` block).

- [ ] **Step 4: Commit**

```bash
git add src/views/Dashboard.vue
git commit -m "feat(views): rewrite Dashboard with cluster hero, quick actions, alerts, active task"
```

### Task 8.2: Translate Machines.vue

**Files:**
- Modify: `src/views/Machines.vue` (full replace)

- [ ] **Step 1: Read source**

Reference: `/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/machines/page.tsx` (already read; 399 lines).

- [ ] **Step 2: Write Vue equivalent following source layout**

Compose with: `UecmPageHeader`, `UecmStatusBadge`, `UecmStatusDot`, `UecmFilterChip`, `UecmRoleTag`, `UecmKV`, `UecmDetailCard`, `UecmCopyText`, `Button`, `Input`, plus existing `MachineDetail.vue` for the right pane (it gets restyled in Phase 9).

**Required selectors (must remain on the new template):**
- `data-discover-btn` — Scan button (top right of list pane)
- `data-create-share-btn` — Share button (next to Scan)
- `data-machine-row` — each row in the list
- `data-machine-check` — per-row checkbox
- `data-select-all` — header checkbox (selects/deselects all)
- `data-batch-env-btn` — Batch env button (multi-select bar)
- `data-batch-ini-btn` — Batch INI button (multi-select bar)

**Stores to wire (do not modify):**
- `useMachinesStore` — `loadMachines()`, `machines`, `selectedDetail`, `selectMachine(id)`, `deleteMachine(id)`, `isLoading`, `error`
- New: read from MACHINES mockData ONLY when `store.machines.length === 0` (empty state demo); otherwise show real store data

**Layout from source (`/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/machines/page.tsx`):**
1. `UecmPageHeader` with title "Machines", subtitle "{n} machines · {online} online · {offline} offline · last LAN scan 4 min ago", crumbs ["UECM", "Machines"], actions: outline Rescan LAN button + primary Add by IP button
2. Filter bar (border-b, bg-card/30, px-6 py-2): search input (w-80) + 4 FilterChips (Role/Health/UE/State) + Saved filters ghost button + sort indicator
3. Grid: list pane (col-span 5/4) + detail pane (col-span 7/8)
4. List pane header row: 4-column grid `24px_1fr_90px_70px` with mono uppercase labels: empty / "Name · IP · Role" / "UE" / "Health"
5. List rows: same grid, each row is a button with `data-machine-row`. Cell 1: StatusDot (healthy if online, offline otherwise). Cell 2: hostname (mono medium 13px) + RoleTag inline + IP/GPU mono 11px muted. Cell 3: UE versions (max 2, mono 10px muted). Cell 4: StatusBadge xs.
6. Bottom of list: "Newly discovered" placeholder row with dashed border-info bg-info/8 + animated radar icon + IP + "Newly discovered · click to add" + Add button
7. Detail pane: import `<MachineDetail>` and pass selected machine

Skeleton:

```vue
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useMachinesStore } from "@/stores/machines";
import MachineDetail from "@/components/machines/MachineDetail.vue";
import DiscoveryWizard from "@/components/modals/DiscoveryWizard.vue";
import CredentialDialog from "@/components/modals/CredentialDialog.vue";
import EnvVarConfigModal from "@/components/modals/EnvVarConfigModal.vue";
import IniEditModal from "@/components/modals/IniEditModal.vue";
import ShareCreateWizard from "@/components/modals/ShareCreateWizard.vue";
import BatchEnvVarModal from "@/components/modals/BatchEnvVarModal.vue";
import BatchIniEditModal from "@/components/modals/BatchIniEditModal.vue";
import {
  UecmPageHeader,
  UecmStatusBadge,
  UecmStatusDot,
  UecmFilterChip,
  UecmRoleTag,
  UecmIcon,
} from "@/components/primitives";
import { Button } from "@/components/ui/Button.vue";
import { Input } from "@/components/ui/Input.vue";

const store = useMachinesStore();
const showDiscovery = ref(false);
const showCredentials = ref(false);
const showEnvVar = ref(false);
const showIniEdit = ref(false);
const showShareWizard = ref(false);
const showBatchEnv = ref(false);
const showBatchIni = ref(false);

const selectedId = computed(() => store.selectedDetail?.machine.id ?? null);
const checkedIds = ref<Set<number>>(new Set());
const checkedArray = computed(() => Array.from(checkedIds.value));

onMounted(() => store.loadMachines());

async function onSelect(id: number | null) {
  if (id === null) return;
  await store.selectMachine(id);
}
async function onDelete(id: number | null) {
  if (id === null) return;
  await store.deleteMachine(id);
  if (id != null) checkedIds.value.delete(id);
}
function toggleCheck(id: number | null) {
  if (id == null) return;
  const next = new Set(checkedIds.value);
  next.has(id) ? next.delete(id) : next.add(id);
  checkedIds.value = next;
}
function toggleAll() {
  const allIds = store.machines.map((m) => m.id).filter((id): id is number => id != null);
  checkedIds.value = checkedIds.value.size === allIds.length ? new Set() : new Set(allIds);
}
const allChecked = computed(() => {
  const total = store.machines.filter((m) => m.id != null).length;
  return total > 0 && checkedIds.value.size === total;
});
</script>

<template>
  <div class="flex h-full flex-col">
    <UecmPageHeader
      title="Machines"
      :subtitle="`${store.machines.length} machines · last LAN scan recent`"
      :crumbs="['UECM', 'Machines']"
    >
      <template #actions>
        <Button
          variant="outline"
          size="sm"
          class="h-8 gap-1.5"
          data-create-share-btn
          @click="showShareWizard = true"
        >
          <UecmIcon name="hard-drive" :size="14" />
          <span class="text-[12px]">Share</span>
        </Button>
        <Button
          variant="outline"
          size="sm"
          class="h-8 gap-1.5"
          data-discover-btn
          @click="showDiscovery = true"
        >
          <UecmIcon name="radar" :size="14" />
          <span class="text-[12px]">Rescan LAN</span>
        </Button>
        <Button size="sm" class="h-8 gap-1.5">
          <UecmIcon name="plus" :size="14" />
          <span class="text-[12px]">Add by IP</span>
        </Button>
      </template>
    </UecmPageHeader>

    <!-- Filter bar -->
    <div class="flex items-center gap-2 border-b border-border bg-card/30 px-6 py-2">
      <div class="relative w-80">
        <UecmIcon name="search" :size="14" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
        <Input placeholder="Search by name or IP…" class="h-8 border-border bg-background pl-8 font-mono text-[12px]" />
      </div>
      <UecmFilterChip label="Role" value="All" />
      <UecmFilterChip label="Health" value="All" />
      <UecmFilterChip label="UE" value="All" />
      <UecmFilterChip label="State" value="All" />
    </div>

    <!-- Multi-select toolbar (only when items selected) -->
    <div
      v-if="store.machines.length > 0"
      class="flex items-center gap-2 border-b border-border bg-card/30 px-6 py-1.5 text-[11px] text-muted-foreground"
    >
      <label class="flex cursor-pointer items-center gap-1.5">
        <input
          data-select-all
          type="checkbox"
          :checked="allChecked"
          @change="toggleAll"
        />
        {{ checkedIds.size }} selected
      </label>
      <Button
        variant="outline"
        size="sm"
        class="h-7 px-2 text-[11px]"
        data-batch-env-btn
        :disabled="checkedIds.size === 0"
        @click="showBatchEnv = true"
      >
        Batch env
      </Button>
      <Button
        variant="outline"
        size="sm"
        class="h-7 px-2 text-[11px]"
        data-batch-ini-btn
        :disabled="checkedIds.size === 0"
        @click="showBatchIni = true"
      >
        Batch INI
      </Button>
    </div>

    <!-- Body: list + detail -->
    <div class="grid flex-1 grid-cols-12 overflow-hidden">
      <section class="col-span-12 flex flex-col overflow-hidden border-r border-border lg:col-span-5 xl:col-span-4">
        <div class="grid grid-cols-[24px_1fr_90px_70px] items-center gap-2 border-b border-border bg-muted/30 px-3 py-2 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
          <span></span>
          <span>Name · IP · Role</span>
          <span>UE</span>
          <span>Health</span>
        </div>
        <div class="flex-1 overflow-y-auto">
          <p v-if="store.isLoading" class="px-3 py-3 text-sm text-muted-foreground">Loading...</p>
          <p v-else-if="store.machines.length === 0" class="px-3 py-3 text-sm text-muted-foreground">
            No machines yet. Click Rescan LAN to discover.
          </p>
          <button
            v-for="m in store.machines"
            :key="m.id ?? m.ip"
            data-machine-row
            :class="[
              'grid w-full grid-cols-[24px_1fr_90px_70px] items-center gap-2 border-b border-border/60 px-3 py-2.5 text-left transition-colors hover:bg-accent/30',
              selectedId === m.id && 'bg-accent/50',
            ]"
            @click="onSelect(m.id)"
          >
            <input
              data-machine-check
              type="checkbox"
              :checked="m.id != null && checkedIds.has(m.id)"
              class="flex-shrink-0"
              @click.stop
              @change="toggleCheck(m.id)"
            />
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <UecmStatusDot :status="m.online !== false ? 'healthy' : 'offline'" :size="6" />
                <span class="truncate font-mono text-[13px] font-medium">{{ m.hostname }}</span>
              </div>
              <div class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground">
                {{ m.ip }}
              </div>
            </div>
            <div class="font-mono text-[10px] text-muted-foreground">—</div>
            <UecmStatusBadge :status="m.online !== false ? 'healthy' : 'offline'" size="xs" />
          </button>
          <p v-if="store.error" class="px-3 py-3 text-[11px] text-status-critical">
            {{ store.error.message }}
          </p>
        </div>
      </section>

      <section class="col-span-12 flex flex-col overflow-hidden lg:col-span-7 xl:col-span-8">
        <MachineDetail
          @open-credential-modal="showCredentials = true"
          @open-env-var-modal="showEnvVar = true"
          @open-ini-edit-modal="showIniEdit = true"
        />
      </section>
    </div>

    <DiscoveryWizard :open="showDiscovery" @close="showDiscovery = false" />
    <CredentialDialog :open="showCredentials" @close="showCredentials = false" />
    <EnvVarConfigModal
      :open="showEnvVar"
      :machine-id="selectedId"
      var-name="UE-SharedDataCachePath"
      @close="showEnvVar = false"
    />
    <IniEditModal :open="showIniEdit" :machine-id="selectedId" @close="showIniEdit = false" />
    <ShareCreateWizard :open="showShareWizard" @close="showShareWizard = false" />
    <BatchEnvVarModal
      :open="showBatchEnv"
      :machine-ids="checkedArray"
      @close="showBatchEnv = false"
    />
    <BatchIniEditModal
      :open="showBatchIni"
      :machine-ids="checkedArray"
      @close="showBatchIni = false"
    />
  </div>
</template>
```

- [ ] **Step 3: Run tests**

```bash
pnpm vitest run src/__tests__/Machines-view.spec.ts src/__tests__/MachineDetail.spec.ts
```

Expected: tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/views/Machines.vue
git commit -m "feat(views): rewrite Machines with new sidebar list + detail layout"
```

### Task 8.3: Translate Projects.vue

**Files:**
- Modify: `src/views/Projects.vue` (full replace)

- [ ] **Step 1: Reference**

Source: `/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/projects/page.tsx` (360 lines).

- [ ] **Step 2: Write Vue equivalent**

**Compose with:** `UecmPageHeader`, `UecmStatusBadge`, `UecmArtifactPill`, `UecmCopyText`, `UecmIcon`, `Button`, `Input`, `RouterLink` (no router for project links yet — placeholder).

**Imports from mockData:** `PROJECTS`, type `Project`.

**Sections (in order):**
1. PageHeader: title "Projects", subtitle "Cross-machine project identity & path mapping", crumbs `["UECM", "Projects"]`, actions: outline Rescan all + primary Add project
2. Two-pane grid (col-span-4 list / col-span-8 detail):
   - **List pane:** Search input at top + scrolling list of project cards. Each card (full-width button, hover bg-accent/30, selected bg-accent/50): name (font-mono 14px semibold) + iniHealth StatusBadge xs + UE / machineCount / lastOp inline mono 11px + 2 ArtifactPills (DDC Pak, PSO) row.
   - **Detail pane:** Header (name + UE pill + INI StatusBadge sm + GUID with CopyText + 4 action buttons: Scan INI / Generate DDC Pak / Collect PSO / Push config). Below: 2-column ArtifactCard grid (DDC Pak + PSO with progress bar, status, distributed/total). Below: Machine Path Mapping card (table with editable path input per row, "manual / discovered / auto" source label, Add machine row, "Saved mapping reuse" footer line). Bottom: Delete project record button (ghost, status-critical color).
3. Local state: `selectedId` ref, defaults to `"exly"`. `project = computed(() => PROJECTS.find(p => p.id === selectedId.value) ?? PROJECTS[0])`.

**Selectors:** None required by existing tests (Projects view has no spec yet — will be added later).

- [ ] **Step 3: Build verify**

```bash
pnpm build 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add src/views/Projects.vue
git commit -m "feat(views): rewrite Projects with mapping editor + artifact pills"
```

### Task 8.4: Translate DDCPak.vue + Wizard

**Files:**
- Modify: `src/views/DDCPak.vue` (full replace)
- Create: `src/components/modals/DDCPakWizard.vue` (NEW — 8-step wizard)

- [ ] **Step 1: Reference**

Source: `/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/ddc-pak/page.tsx` (487 lines, includes WizardModal inline).

- [ ] **Step 2: Write DDCPak.vue**

**Compose with:** `UecmPageHeader`, `UecmStatusBadge`, `UecmSectionHeader`, `UecmStatusDot`, `UecmIcon`, `Button`, plus new local component `DDCPakWizard` (Step 3).

**Imports:** `useTasksStore`, `HISTORY_TASKS`, type `TaskItem`.

**Sections (in order):**
1. PageHeader: title "DDC Pak", subtitle "Generate and distribute Derived Data Cache packs across the cluster", crumbs `["UECM", "DDC Pak"]`, actions: primary "One-stop Wizard" button opens `wizardOpen = true`.
2. 3 PrimaryAction cards row (md:grid-cols-3): "New Generate" / "New Distribute" / "One-stop Wizard". Each is a card with 32×32 icon square (border + bg-background + text-primary), 13px semibold mono uppercase title, 12px muted desc, outline/primary CTA button with chevron-right. The third card has primary tint (`border-primary/40 bg-[color-mix(in_oklch,var(--primary)_8%,var(--card))]`).
3. Running tasks section: `UecmSectionHeader title="Running tasks" :count="tasks.running.length"`, then 2-col grid of `RunningTaskCard` per running task. Each card has status-info border + tint bg, header row (StatusBadge progress + project name + elapsed/ETA), stage line, progress bar, target chips grid (2-4 cols, status dot per chip — first 4 healthy, 5th info+pulse, rest muted, demonstrative), Pause/Cancel/Full log row.
4. History section: `UecmSectionHeader title="Task history" :count="HISTORY_TASKS.length"`, then table (full-width, border rounded, mono uppercase tracking-wider header). Columns: Time / Type / Project / Source / Targets / Duration / Status (StatusBadge xs from `t.status`) / actions (Re-run ghost button).

- [ ] **Step 3: Write DDCPakWizard.vue**

**Standalone modal — does NOT use BaseModal** (different size + custom step indicator + custom footer with checkboxes).

**Compose with:** `Teleport`, `UecmIcon`, `Button`. Inline cn() helper.

**Sections:**
1. Backdrop: `<div v-if="open" class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" @click.self="$emit('close')">`.
2. Container: `flex h-[640px] w-[920px] max-w-[95vw] flex-col rounded-xl border border-border bg-card shadow-2xl`.
3. Header (h-12, border-b, px-5): wand-2 icon + "DDC Pak · One-stop Wizard" mono uppercase + Close X button.
4. Step indicator (border-b, bg-muted/20, px-5 py-3): horizontal flex, 8 steps from `WIZARD_STEPS = ["Mode", "Source", "Project", "Parameters", "Targets", "Path mapping", "Preview", "Execute"]`. Each step: 5×5 rounded-full badge (active: border-primary bg-primary text-primary-foreground; done: border-status-healthy bg-status-healthy text-background with check icon; pending: border-border bg-muted text-muted-foreground) + uppercase label + chevron-right between.
5. Body: scrollable. Demo at step=6 (Preview): 12-col grid. Left col-span-8: Shield icon + "Preview as Contract" header + project/source meta. Two `<pre>` code blocks (UE command line + PowerShell distribution plan) with copy buttons. Warning callout (status-warning tint border + bg) about RENDER-05 skipped. Right col-span-4: "Affected machines" list card (mock 7 healthy machines + 1 offline strikethrough) + "Files affected" card with mono path list.
6. Footer (h-14, border-t, px-5): left side has Auto backup checkbox (defaultChecked) + Dry Run checkbox + "Reversible: partial" mono. Right side: Cancel ghost / Back outline (chevron-left) / Execute primary (chevron-right).

**State:** `open` prop, `step` ref defaulting to 6 (preview demo), `setStep` function.

- [ ] **Step 4: Commit**

```bash
git add src/views/DDCPak.vue src/components/modals/DDCPakWizard.vue
git commit -m "feat(views): rewrite DDC Pak + add 8-step One-stop Wizard"
```

### Task 8.5: Translate PSOCache.vue

**Files:**
- Modify: `src/views/PSOCache.vue` (full replace)

- [ ] **Step 1: Reference**

Source: `/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/pso-cache/page.tsx` (344 lines).

- [ ] **Step 2: Write Vue equivalent**

**Compose with:** `UecmPageHeader`, `UecmStatusBadge`, `UecmStatusDot`, `UecmIcon`, `Button`.

**Imports:** `MACHINES` from mockData, type `Machine`.

**Helper (inline at top of `<script setup>`):**

```ts
function groupByGpu(machines: Machine[]) {
  const groups = new Map<string, Machine[]>();
  for (const m of machines) {
    const key = `${m.gpu}__${m.driver}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(m);
  }
  return Array.from(groups.entries()).sort((a, b) => b[1].length - a[1].length);
}
const groups = groupByGpu(MACHINES);
const dominant = groups[0];
const minorGroups = groups.slice(1);
```

**Sections (in order):**
1. PageHeader: title "PSO Cache", subtitle "Collect and distribute PSO cache; verify GPU / driver consistency", crumbs `["UECM", "PSO Cache"]`, actions: outline "Verify Precaching" + primary "Collect PSO wizard".
2. **GPU / Driver Consistency** card (rounded-lg border bg-card): h-10 header with "GPU / Driver Consistency" mono uppercase + StatusBadge warning "MIXED" + group/machine count meta. Body: 3-col grid (gap-px bg-border) of group cards. Each: gpu name (mono semibold 12px), driver + driverDate, "DOMINANT" or "MINORITY" StatusBadge, machine chips wrapping (mono 10px chip per machine, online=healthy dot, offline=offline dot+strikethrough), "{n} machine(s) can share the same PSO Cache" footer.
3. **Recommendation callout** (when minorGroups.length > 0): border-t, bg-status-warning/6 tint. AlertTriangle icon + "{n} machine(s) cannot share..." headline + per-group breakdown (GPU/driver mismatch lines) + right side "Suggested: upgrade ... to driver {dominant.driver}" lightbulb chip + primary "Apply suggestion" button.
4. **Operations row** (3-col grid): OpCard each — Collect PSO / Distribute PSO / Verify Precaching. Each is a card with 28×28 icon square + mono uppercase title + 12px muted desc + outline button.
5. **Active Collection** card: h-10 header "Active Collection" mono uppercase + StatusBadge progress label "EDITOR-01 · VP_Demo" + elapsed mono. Body 3-col grid: left col-span-2 has warning callout (Manual scene traversal required) + UE command line `<pre>` block. Right col-span-1: 3 stat tiles (UE process / PSOs collected / Saved/CollectedPSOs) + Open log Button.
6. **CVar verification** card: h-10 header "PSO Precaching CVar State" + ghost "One-click setup" button. Table with columns Machine / r.PSOPrecaching / r.PSOPrecaching.Validation / r.ShaderPipelineCache.LastFile / Status. First 7 machines from MACHINES, render `<code>` boxes for CVar values (highlight warning when not healthy).

- [ ] **Step 3: Commit**

```bash
git add src/views/PSOCache.vue
git commit -m "feat(views): rewrite PSO Cache with GPU/driver group matrix + collection monitor + CVar table"
```

### Task 8.6: Translate INIScanner.vue (with diff blocks)

**Files:**
- Modify: `src/views/INIScanner.vue` (full replace)

- [ ] **Step 1: Reference**

Source: `/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/ini-scanner/page.tsx` (451 lines).

- [ ] **Step 2: Write Vue equivalent**

**Compose with:** `UecmPageHeader`, `UecmStatusBadge`, `UecmFilterChip`, `UecmCodeBlock`, `UecmIcon`, `Button`.

**Imports:** `INI_FINDINGS`, type `IniFinding` from mockData.

**Helper (inline):**

```ts
function groupBy<T>(arr: T[], fn: (t: T) => string): Record<string, T[]> {
  const out: Record<string, T[]> = {};
  for (const item of arr) {
    const k = fn(item);
    if (!out[k]) out[k] = [];
    out[k].push(item);
  }
  return out;
}
```

**State:** `grouping = ref<"machine" | "project">("machine")`, `selectedId = ref("f-1")`, `finding = computed(...)`.

**Sections (in order):**
1. PageHeader: title "INI Scanner", subtitle "Scanned {total} ini files across cluster · last scan 4 min ago", crumbs `["UECM", "INI Scanner"]`, actions: outline Rescan all / outline Export report / primary Fix all (with wrench icon).
2. Summary strip (4-cell grid bg-border gap-px): "Health" pct healthy + "Critical" + "Warning" + "Healthy" cells. Each: bg-card px-6 py-3, mono uppercase label + tone-colored value + sub line.
3. Filter bar (border-b bg-card/30 px-6 py-2): Grouping toggle (2 buttons inside a bordered group, "Machine → Project" / "Project → Machine", active state bg-accent text-foreground) + 4 FilterChips (Severity / Issue type / Machine / Project) + Saved filters ghost button.
4. Two-pane grid (col-span-5/4 hierarchy / col-span-7/8 detail):
   - **Hierarchy pane:** recursive `groupBy` rendering. Top level (machine OR project, depending on `grouping`): sticky header with chevron-down + Server/Folder icon + name + crit/warn count badges. Mid level: indented (pl-7), chevron-down + opposite icon + count. File level: pl-11, FileText icon + filename. Finding rows: pl-14, full-width buttons with StatusBadge xs (label="C" or "W") + rule + file:line in mono 10px + chevron-right. Selected row gets `bg-accent/60`.
   - **Detail pane:** Header (border-b bg-card/30 px-6 py-4): StatusBadge sm + h2 finding.rule + machine/project meta. Path line: FolderOpen icon + truncated full path + line number + Copy icon. Body (overflow-y-auto p-6): 3-col DiagBlock grid ("What's wrong" status-critical tint, "Why it matters" plain border, "User-facing symptom" status-warning tint with Skull icon prefix). 2-col `UecmCodeBlock` diff: left = critical tone "Detected (current)" using `finding.before`, right = healthy tone "Suggested fix" using `finding.after`, both `:start-line="finding.line - 4"`, both `:highlight-line="finding.line"`. Repair actions card (rounded-lg border bg-card p-4): "Repair" mono + "auto-backup enabled · reversible" sub + 4 buttons row (primary "Apply suggested fix" with wrench / outline "Custom edit" / outline "Open file" / ghost "Skip finding") + state meta on right.

**Selectors:** None required.

- [ ] **Step 3: Commit**

```bash
git add src/views/INIScanner.vue
git commit -m "feat(views): rewrite INI Scanner with hierarchy + 3-col diagnostic + before/after diff"
```

### Task 8.7: Translate HealthCheck.vue (Matrix + Console)

**Files:**
- Modify: `src/views/HealthCheck.vue` (full replace)

- [ ] **Step 1: Reference**

Source: `/Users/bip.lan/Downloads/b_sVGHApfkUd9/app/health-check/page.tsx` (586 lines, includes CHECK_DETAIL data, MatrixView, ConsoleView, CheckDetail).

- [ ] **Step 2: Port CHECK_DETAIL data**

Add the `CHECK_DETAIL` map (102 lines) to `src/lib/mockData.ts` as a new export.

- [ ] **Step 3: Write Vue equivalent**

**Compose with:** `UecmPageHeader`, `UecmStatusBadge`, `UecmStatusDot`, `UecmMatrixCell`, `UecmKpiTile`, `UecmScoreTile`, `UecmIcon`, `Button`.

**Imports:** `MACHINES`, `HEALTH_CHECKS`, `CHECK_DETAIL` from mockData (the latter added in Step 2). Type `StatusKind`, `Machine`.

**Helper functions (inline):**

```ts
function shortLabel(label: string) {
  return label
    .replace("Firewall ", "FW ")
    .replace("Shared drive reachable", "Share")
    .replace("NTFS permission (Host only)", "NTFS")
    .replace("User-level credential", "Cred U")
    .replace("SYSTEM-level credential", "Cred SYS")
    .replace("Environment variables", "Env")
    .replace("INI consistency", "INI")
    .replace("SYSTEM write test", "SYS Write")
    .replace("PSO Precaching CVar", "PSO cvar")
    .replace("GPU / driver consistency", "GPU")
    .replace("SMB service running", "SMB");
}

function countTotals() {
  let healthy = 0, warning = 0, critical = 0, offline = 0, total = 0;
  for (const m of MACHINES) {
    for (const c of HEALTH_CHECKS) {
      const s = m.checks[c.id];
      if (!s || s === "na") continue;
      total++;
      if (s === "healthy") healthy++;
      else if (s === "warning") warning++;
      else if (s === "critical") critical++;
      else if (s === "offline") offline++;
    }
  }
  return { healthy, warning, critical, offline, total };
}

function sampleOutput(id: string, status: StatusKind, m: Machine): string {
  // Same body as React source's sampleOutput. Returns multi-line probe sample.
}
```

**State:** `running = ref(false)`, `mode = ref<"matrix" | "console">("matrix")`, `sel = ref<{machineId: string; checkId: string} | null>({machineId: "render-02", checkId: "sysWrite"})`. `runFull` function sets running=true for 2400ms then false.

**Sections (in order):**
1. PageHeader: title "Health Check", subtitle "11 checks · 10 hosts · last full run 2026-05-02 13:55:08 (2m 31s)", crumbs `["UECM", "Health Check"]`, actions: outline "Recheck failed" / primary "Run full diagnostics" (with Loader2 spin when running, PlayCircle otherwise).
2. KPI strip (5-cell border-b bg-border gap-px): UecmScoreTile "Cluster Score" 70 warning DEGRADED + 4 KpiTiles (Healthy / Warning / Critical / Offline using countTotals() values).
3. Tabs (border-b bg-card/30 px-4): 2 TabButtons "Matrix" (Activity icon) / "Console" (Terminal icon). Active tab has border-b-2 border-primary text-foreground; inactive has border-transparent text-muted-foreground. Right side: "Probing 10 machines × 11 checks…" or "Idle" mono.
4. Body: switch on `mode`.

**MatrixView:**
- 12-col grid: matrix col-span-8 (overflow-auto border-r border-border), detail aside col-span-4 (overflow-y-auto bg-card/30).
- Matrix table: `min-w-[920px] border-collapse`. Sticky thead, 1st col 180px sticky-left "Machine" header. 11 check headers using `shortLabel()`, emphasized checks (Cred SYS, SYS Write) get bottom 0.5×4 primary/70 underline.
- Each row: sticky-left machine cell with StatusDot + name (mono 12px medium) + role/IP (mono 10px muted). 11 `<UecmMatrixCell>` cells per row, with `:status="m.checks[c.id] ?? 'unknown'"`, `:emphasized="c.emphasized"`, `:selected="sel?.machineId === m.id && sel?.checkId === c.id"`, `@click="sel = {machineId: m.id, checkId: c.id}"`.
- Detail aside: if `sel` is null, show EmptyDetail (Activity icon + "Select a cell" + Chinese hint paragraph). Otherwise: header (StatusBadge md + "Critical Path" badge if emphasized + check.label + machine meta) + 3 DetailSections (What/How [mono]/Symptom) + "Suggested Fix" wrench section with mono `<pre>` block from `detail.fix.map(...)` + Last probe output mono `<pre>` block tone-colored from `sampleOutput()`. Footer (sticky bottom, border-t bg-card/60): outline Re-run + primary Apply Auto-fix.

**ConsoleView:**
- Header bar (border-b bg-card/40 px-4 py-2): pulsing dot (info if running, muted otherwise) + "Probing"/"Idle" + "tools/uecm-healthcheck.ps1" + Copy log button right.
- `<pre>` body (flex-1 overflow-y-auto bg-background px-4 py-3): each line is flex gap-3, 24-wide muted timestamp + 12-wide tone-colored level (INFO=muted / OK=healthy / WARN=warning / ERR=critical) + message in foreground/90.
- Trailing animated "running probes…" line if running.
- Use `consoleLines(running)` helper that returns 15 mock lines (from the React source). When running, return only first 6.

- [ ] **Step 4: Commit**

```bash
git add src/views/HealthCheck.vue src/lib/mockData.ts
git commit -m "feat(views): rewrite Health Check with Matrix + Console modes + check details"
```

### Task 8.8: Restyle Shares.vue

**Files:**
- Modify: `src/views/Shares.vue`

- [ ] **Step 1: Read current**

```bash
cat src/views/Shares.vue
```

- [ ] **Step 2: Replace styling to match new design**

Use `UecmPageHeader`, replace ad-hoc Tailwind colors with new tokens (`bg-card`, `text-foreground`, `border-border`, `text-status-warning` etc.). Preserve all existing `useSharesStore` wiring and `data-*` selectors.

- [ ] **Step 3: Run tests**

```bash
pnpm vitest run src/__tests__/Shares-view.spec.ts
```

Expected: 4 PASS.

- [ ] **Step 4: Commit**

```bash
git add src/views/Shares.vue
git commit -m "feat(views): restyle Shares to new design tokens (kept as route, accessible from Dashboard)"
```

### Task 8.9: Verify all views render

- [ ] **Step 1: Build + full test**

```bash
pnpm build 2>&1 | tail -10
pnpm test 2>&1 | tail -15
```

Expected: build passes; all 22 spec files pass.

- [ ] **Step 2: Manually verify in dev**

```bash
pnpm dev
```

Open `http://127.0.0.1:5173/`, click through each route in the sidebar. Check both light and dark themes via the toggle. Ensure no console errors.

---

## Phase 9 — Restyle Existing Modals (45 min)

Update `BaseModal.vue` and 7 existing modals to match new design tokens. Preserve all stores and `data-*` selectors.

### Task 9.1: BaseModal — accept size prop + new tokens

**Files:**
- Modify: `src/components/modals/BaseModal.vue`

- [ ] **Step 1: Read current**

```bash
cat src/components/modals/BaseModal.vue
```

- [ ] **Step 2: Replace contents**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/utils";

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string;
    size?: "sm" | "md" | "lg" | "xl";
  }>(),
  { size: "md" }
);

const emit = defineEmits<{ close: [] }>();

const sizeCls = computed(() => ({
  sm: "w-[400px]",
  md: "w-[480px]",
  lg: "w-[640px]",
  xl: "w-[920px] h-[640px]",
}[props.size]));
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      data-base-modal
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      @click.self="emit('close')"
    >
      <div
        :class="cn('flex max-w-[95vw] flex-col rounded-xl border border-border bg-card shadow-2xl', sizeCls)"
      >
        <header
          v-if="title || $slots.header"
          class="flex items-center justify-between border-b border-border px-5 py-3"
        >
          <slot name="header">
            <h2 class="font-mono text-[13px] font-semibold uppercase tracking-wide">
              {{ title }}
            </h2>
          </slot>
          <button
            class="text-muted-foreground hover:text-foreground"
            aria-label="Close"
            @click="emit('close')"
          >
            ×
          </button>
        </header>
        <div class="flex-1 overflow-y-auto p-5">
          <slot />
        </div>
        <footer
          v-if="$slots.footer"
          class="flex items-center justify-end gap-2 border-t border-border px-5 py-3"
        >
          <slot name="footer" />
        </footer>
      </div>
    </div>
  </Teleport>
</template>
```

- [ ] **Step 3: Run BaseModal tests**

```bash
pnpm vitest run src/__tests__/BaseModal.spec.ts
```

Expected: 6 PASS.

### Task 9.2: Restyle 7 existing modals

**Files:**
- Modify: `src/components/modals/CredentialDialog.vue`
- Modify: `src/components/modals/DiscoveryWizard.vue`
- Modify: `src/components/modals/EnvVarConfigModal.vue`
- Modify: `src/components/modals/IniEditModal.vue`
- Modify: `src/components/modals/ShareCreateWizard.vue`
- Modify: `src/components/modals/BatchEnvVarModal.vue`
- Modify: `src/components/modals/BatchIniEditModal.vue`

For each: read current → replace ad-hoc styles with new tokens → keep all store wiring + `data-*` selectors → run respective spec.

- [ ] **Step 1: One commit per modal — example for CredentialDialog**

```bash
pnpm vitest run src/__tests__/CredentialDialog.spec.ts
```

After confirming the modal still passes, commit:

```bash
git add src/components/modals/CredentialDialog.vue
git commit -m "style(modals): restyle CredentialDialog to new design tokens"
```

Repeat for each of the 6 remaining modals.

### Task 9.3: Wire Dashboard "Create Shared DDC" card to ShareCreateWizard

**Files:**
- Modify: `src/views/Dashboard.vue`

- [ ] **Step 1: Add wiring**

Replace the static "Create Shared DDC" button in Dashboard with one that opens `ShareCreateWizard`:

```vue
<script setup lang="ts">
// ... existing imports
import ShareCreateWizard from "@/components/modals/ShareCreateWizard.vue";
import { ref } from "vue";

const showShareWizard = ref(false);
</script>
```

Modify the first quick action button to set `showShareWizard = true` on click. Add the modal at the end:

```vue
<ShareCreateWizard :open="showShareWizard" @close="showShareWizard = false" />
```

- [ ] **Step 2: Run dashboard tests**

```bash
pnpm vitest run src/__tests__/Dashboard-view.spec.ts
```

Expected: 2 PASS (existing tests don't test this wiring; it's purely additive).

- [ ] **Step 3: Commit**

```bash
git add src/views/Dashboard.vue
git commit -m "feat(dashboard): wire Create Shared DDC card to ShareCreateWizard modal"
```

---

## Phase 10 — Final Verification (15 min)

### Task 10.1: Full test sweep

- [ ] **Step 1: Run full vitest**

```bash
pnpm test 2>&1 | tail -20
```

Expected: 22+ test files, ~100 tests passing.

### Task 10.2: Production build

- [ ] **Step 1: Build**

```bash
pnpm build 2>&1 | tail -15
```

Expected: clean build, fonts bundled (Manrope + Inter), CSS ~25-35 KB, JS ~200-280 KB.

### Task 10.3: Manual smoke test in browser

- [ ] **Step 1: Run dev server**

```bash
pnpm dev
```

- [ ] **Step 2: Walk through each route in dark mode**

Visit:
- `/` (Dashboard)
- `/machines`
- `/projects`
- `/ddc-pak` (open wizard)
- `/pso-cache`
- `/ini-scanner`
- `/health-check` (toggle Matrix/Console)
- `/shares` (verify still accessible)

Check for: console errors, layout breakage, missing icons, broken images.

- [ ] **Step 3: Toggle theme**

Use the topbar theme toggle. Switch light → dark → system. Verify no flash, all status colors readable in both modes.

- [ ] **Step 4: Toggle log panel**

Click "Logs" in top bar. Panel slides up from bottom. Click "Hide" — panel disappears.

- [ ] **Step 5: Active task pill on each view**

Confirm the topbar active-task pill (DISTRIBUTE · EXLY · 64%) is visible on every page.

### Task 10.4: Summary commit + tag

- [ ] **Step 1: Tag the milestone**

```bash
git tag -a design-system-overhaul -m "Complete UECM design system overhaul: dual theme, new sidebar/topbar/log panel, 7 view rewrites, 8 modal restyles, tasks store, mock data."
git log --oneline | head -30
```

---

## Self-Review Checklist (run after writing each task)

- [ ] **Spec coverage:** Every section of the new design source is implemented in some task.
- [ ] **No placeholders:** Every step shows actual code or actual command. No "TBD" or "implement later".
- [ ] **Type consistency:** `Tone` type matches in all primitive components; `MachineRole` matches in mockData and UecmRoleTag; `StatusKind` consistent across stores and views.
- [ ] **Selectors preserved:** All `data-*` attributes used by existing tests remain on the rewritten components (verified by passing test runs after each task).
- [ ] **Stores untouched:** `useMachinesStore`, `useDiscoveryStore`, `useCredentialsStore`, `useSharesStore`, `useBatchStore` are not modified — only added two new stores (`tasks`, `cluster`).
- [ ] **Routes intact:** `/`, `/machines`, `/shares`, `/projects`, `/ddc-pak`, `/pso-cache`, `/ini-scanner`, `/health-check` still resolvable.

---

## Total Effort Estimate

| Phase | Time |
|---|---|
| 1 — Foundation reset | 45 min |
| 2 — Theme switching | 30 min |
| 3 — shadcn-vue minimal port | 40 min |
| 4 — Theme toggle | 15 min |
| 5 — Mock data + stores | 45 min |
| 6 — UECM primitives | 90 min |
| 7 — Global shell rebuild | 90 min |
| 8 — 7 view translations | 3.5–4 hrs |
| 9 — Modals restyle | 45 min |
| 10 — Final verification | 15 min |
| **Total** | **~9–10 hrs** |
