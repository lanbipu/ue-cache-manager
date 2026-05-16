import animate from "tailwindcss-animate";

// Helper: tokens.css stores bare OKLCH components ("L C H"); wrap with
// `oklch(... / <alpha-value>)` so Tailwind generates alpha modifiers
// (`bg-muted/40`, `hover:bg-primary/90`, etc.).
const ok = (varName) => `oklch(var(${varName}) / <alpha-value>)`;

/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{vue,ts,tsx}"],
  theme: {
    extend: {
      colors: {
        background: ok("--background"),
        foreground: ok("--foreground"),
        card: {
          DEFAULT: ok("--card"),
          foreground: ok("--card-foreground"),
        },
        popover: {
          DEFAULT: ok("--popover"),
          foreground: ok("--popover-foreground"),
        },
        primary: {
          DEFAULT: ok("--primary"),
          foreground: ok("--primary-foreground"),
        },
        secondary: {
          DEFAULT: ok("--secondary"),
          foreground: ok("--secondary-foreground"),
        },
        muted: {
          DEFAULT: ok("--muted"),
          foreground: ok("--muted-foreground"),
        },
        accent: {
          DEFAULT: ok("--accent"),
          foreground: ok("--accent-foreground"),
        },
        destructive: {
          DEFAULT: ok("--destructive"),
          foreground: ok("--destructive-foreground"),
        },
        sidebar: {
          DEFAULT: ok("--sidebar"),
          foreground: ok("--sidebar-foreground"),
          primary: ok("--sidebar-primary"),
          "primary-foreground": ok("--sidebar-primary-foreground"),
          accent: ok("--sidebar-accent"),
          "accent-foreground": ok("--sidebar-accent-foreground"),
          border: ok("--sidebar-border"),
        },
        status: {
          healthy: ok("--status-healthy"),
          warning: ok("--status-warning"),
          critical: ok("--status-critical"),
          info: ok("--status-info"),
          offline: ok("--status-offline"),
          unknown: ok("--status-unknown"),
          online: ok("--status-healthy"),
          warn: ok("--status-warning"),
        },
        surface: {
          DEFAULT: ok("--card"),
          raised: ok("--popover"),
          subtle: ok("--muted"),
          inverse: ok("--foreground"),
        },
        fg: {
          1: ok("--foreground"),
          2: ok("--muted-foreground"),
          3: ok("--muted-foreground"),
        },
      },
      borderColor: {
        DEFAULT: ok("--border"),
        input: ok("--input"),
        ring: ok("--ring"),
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
        ui: "var(--font-sans)",
      },
      ringColor: {
        DEFAULT: ok("--ring"),
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
