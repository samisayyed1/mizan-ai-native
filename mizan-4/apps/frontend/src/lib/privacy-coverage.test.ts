// Regression guard for privacy-mode coverage across the app.
//
// Background:
//   The dashboard has a privacy toggle (the "eye" icon) that hides
//   every monetary value behind ••••. The mechanism is per-cell —
//   each `<AmountDisplay>` reads `isBalanceHidden` from
//   `useBalancePrivacy()` and renders ••••.
//
// The bug we found:
//   9 production call sites of `<AmountDisplay>` did NOT pass
//   `isHidden`, so when the user toggled privacy mode, money leaked
//   through in 9 spots — holdings table price column, allocation
//   detail sheet (3 rows), and activity detail sheet (5 rows).
//
// What this test does:
//   Walks every .tsx file under apps/frontend/src and packages/ui/src,
//   finds every `<AmountDisplay …>` tag, and asserts that each tag
//   either carries an `isHidden` prop (passed through) or sits in a
//   demonstrably non-leaking context (an explicit safe-listed file —
//   today none, kept here for the day a legitimate exception arises).
//
//   If a new contributor adds a `<AmountDisplay>` without isHidden,
//   this test fails CI with the exact file:line that leaked.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Directories scanned. Anything under here that uses AmountDisplay is
// in scope.
const ROOTS = [
  resolve(__dirname, "..", "..", "..", "frontend", "src"),
  resolve(__dirname, "..", "..", "..", "..", "packages", "ui", "src"),
];

// Files allowed to render AmountDisplay without isHidden, with a
// short reason. Empty today — the rule is: pass isHidden everywhere.
// If a future case is genuinely a never-private surface (e.g. a
// public-facing marketing component, NOT a user dashboard surface),
// add it here with a reason and a code comment in that file.
const ALLOWLIST: readonly { file: string; reason: string }[] = [];

function walk(dir: string, out: string[] = []): string[] {
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const name of entries) {
    if (name === "node_modules" || name.startsWith(".")) continue;
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) walk(full, out);
    else if (st.isFile() && (full.endsWith(".tsx") || full.endsWith(".ts"))) out.push(full);
  }
  return out;
}

function findLeakySites(file: string): number[] {
  const text = readFileSync(file, "utf8");
  const leaks: number[] = [];
  // Walk through the file looking for `<AmountDisplay` and the
  // matching tag close (`/>` or `>`). Capture the tag body and
  // check for the `isHidden` substring inside it.
  let i = 0;
  while (true) {
    const open = text.indexOf("<AmountDisplay", i);
    if (open === -1) break;
    // Find the end of the opening tag — the first `>` not preceded
    // by an open brace counter > 0 (to handle props like
    // `value={something > 0 ? a : b}` which contain `>`).
    let depth = 0;
    let end = -1;
    for (let p = open + "<AmountDisplay".length; p < text.length; p++) {
      const ch = text[p];
      if (ch === "{") depth++;
      else if (ch === "}") depth--;
      else if (ch === ">" && depth === 0) {
        end = p;
        break;
      }
    }
    if (end === -1) break;
    const body = text.slice(open, end + 1);
    if (!body.includes("isHidden")) {
      // Compute the line number.
      const line = text.slice(0, open).split("\n").length;
      leaks.push(line);
    }
    i = end + 1;
  }
  return leaks;
}

describe("privacy-mode coverage on <AmountDisplay>", () => {
  it("every production <AmountDisplay> passes isHidden (or is allowlisted)", () => {
    const seen: { file: string; lines: number[] }[] = [];
    for (const root of ROOTS) {
      for (const file of walk(root)) {
        // Skip test files — they explicitly test variants.
        if (/\.test\.(ts|tsx)$/.test(file)) continue;
        // Skip the AmountDisplay component itself — it defines the prop.
        if (file.endsWith("/amount-display.tsx")) continue;

        const allowed = ALLOWLIST.find((entry) => file.endsWith(entry.file));
        if (allowed) continue;

        const leaks = findLeakySites(file);
        if (leaks.length > 0) seen.push({ file, lines: leaks });
      }
    }

    if (seen.length > 0) {
      const detail = seen.map((s) => `  ${s.file}:${s.lines.join(",")}`).join("\n");
      throw new Error(
        `Found ${seen.length} file(s) with <AmountDisplay> missing isHidden.\n` +
          `Pass isHidden={isBalanceHidden} from useBalancePrivacy() at each site.\n\n` +
          detail,
      );
    }

    expect(seen).toEqual([]);
  });
});
