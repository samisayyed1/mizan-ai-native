import type { Config } from "tailwindcss";

/**
 * Mizan landing — brand tokens ported verbatim from the desktop's
 * `mizan-4/apps/frontend/src/globals.css`. Do not invent variants.
 *
 *   --gold-cream:   #F5E6C8  HSL 40 67% 87%   (headline emphasis, wordmark tittle)
 *   --gold-primary: #D4A574  HSL 31 49% 64%   (CTAs, focus rings, accents)
 *   --gold-deep:    #8B6F47  HSL 31 32% 41%   (dividers, micro-text on dark)
 *
 *   --depth-page:      hsl(0 0% 4.5%)
 *   --depth-container: hsl(0 0% 6.5%)
 *   --depth-card:      hsl(0 0% 9%)
 *   --depth-elevated:  hsl(0 0% 12%)
 *
 *   --foreground:  hsl(55 10% 79%)  (Flexoki tx, dark variant)
 *   --success:     hsl(72 46% 41%)
 *   --destructive: hsl(5 61% 54%)
 *   --warning:     hsl(45 82% 45%)
 */
const config: Config = {
  content: ["./app/**/*.{ts,tsx,mdx}", "./emails/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        gold: {
          cream: "hsl(40 67% 87%)",
          primary: "hsl(31 49% 64%)",
          deep: "hsl(31 32% 41%)",
        },
        depth: {
          page: "hsl(0 0% 4.5%)",
          container: "hsl(0 0% 6.5%)",
          card: "hsl(0 0% 9%)",
          elevated: "hsl(0 0% 12%)",
          border: "rgba(255, 255, 255, 0.06)",
        },
        foreground: "hsl(55 10% 79%)",
        success: "hsl(72 46% 41%)",
        destructive: "hsl(5 61% 54%)",
        warning: "hsl(45 82% 45%)",
      },
      fontFamily: {
        sans: ["var(--font-sans)"],
        serif: ["var(--font-serif)"],
        mono: ["var(--font-mono)"],
      },
      // 8-pt spacing rhythm (same rule as mizan-4's PR-DENSITY-7).
      // Tailwind's default scale already gives us 4/8/12/16/20/24/32/40/48/64.
      maxWidth: {
        "container": "72rem", // ~1152px — matches mizan-4 max-w-6xl
      },
    },
  },
  plugins: [],
};
export default config;
