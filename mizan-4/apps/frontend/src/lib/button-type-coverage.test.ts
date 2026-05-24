// Regression guard for `<button>` `type=` discipline.
//
// Background:
//   HTML buttons default to `type="submit"`. When a `<button>` lives
//   inside a `<form>` (and we use `react-hook-form` heavily — most
//   pages have a form somewhere in the ancestor chain), an
//   un-typed `<button>` accidentally submits the form on click. In
//   the onboarding flow this silently broke the theme / currency /
//   timezone selectors: the user would tap a button to pick "Dark"
//   and onboarding would lurch forward without saving anything.
//
//   We swept all 45 production sites and added `type="button"` to
//   each. This test ensures that fix doesn't regress.
//
// What this test does:
//   Walks every .tsx / .jsx file under apps/frontend/src, finds every
//   `<button …>` opening tag (handling JSX expression braces in
//   attribute values), and asserts each one carries an explicit
//   `type=` attribute. Test files (`.test.tsx`, `__tests__/`) are
//   excluded — they sometimes test exact markup intentionally.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const ROOT = resolve(__dirname, "..", "..", "..", "frontend", "src");

function walk(dir: string, out: string[] = []): string[] {
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const name of entries) {
    if (name === "node_modules" || name.startsWith(".")) continue;
    if (name === "__tests__") continue;
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) walk(full, out);
    else if (st.isFile() && (full.endsWith(".tsx") || full.endsWith(".jsx"))) {
      if (/\.test\.(tsx|jsx)$/.test(full)) continue;
      out.push(full);
    }
  }
  return out;
}

/**
 * Find every `<button …>` opening tag in `text` that does NOT carry
 * a `type=` attribute. Returns 1-based line numbers. Handles JSX
 * expression braces in attribute values (e.g. `onClick={() => x > 0
 * && y}`) by tracking brace depth so we only stop at the `>` that
 * closes the opening tag.
 */
function findUntypedButtons(text: string): number[] {
  const re = /<button(?=[\s>])/g;
  const lines: number[] = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    const start = m.index;
    let depth = 0;
    let end = -1;
    for (let p = start + "<button".length; p < text.length; p++) {
      const ch = text[p];
      if (ch === "{") depth++;
      else if (ch === "}") depth--;
      else if (ch === ">" && depth === 0) {
        end = p;
        break;
      }
    }
    if (end === -1) break;
    const tag = text.slice(start, end + 1);
    if (!/\btype=/.test(tag)) {
      const line = text.slice(0, start).split("\n").length;
      lines.push(line);
    }
  }
  return lines;
}

describe("button-type discipline", () => {
  it("every production <button> opener carries an explicit type= attribute", () => {
    const offenders: { file: string; lines: number[] }[] = [];
    for (const file of walk(ROOT)) {
      const text = readFileSync(file, "utf8");
      const lines = findUntypedButtons(text);
      if (lines.length > 0) offenders.push({ file, lines });
    }
    if (offenders.length > 0) {
      const detail = offenders.map((o) => `  ${o.file}: ${o.lines.join(",")}`).join("\n");
      throw new Error(
        `Found ${offenders.length} file(s) with <button> elements missing type=.\n` +
          `Default HTML type is "submit" — inside a <form>, these silently submit on click.\n` +
          `Add type="button" (or type="submit" if that's actually intended) to each.\n\n` +
          detail,
      );
    }
    expect(offenders).toEqual([]);
  });
});
