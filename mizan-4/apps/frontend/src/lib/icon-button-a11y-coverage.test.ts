// Regression guard for icon-only-button accessible names.
//
// Background:
//   A `<Button size="icon">` with only an icon child is rendered as a
//   `<button>` containing nothing but an `<svg>`. Screen readers
//   announce these as "button" with no clue what they do. The fix is
//   to add `aria-label` (or `aria-labelledby` / `title`), or to put
//   the button inside a `<TooltipTrigger asChild>` whose tooltip
//   content names it.
//
//   We swept all 21 production icon-only buttons that were missing
//   accessible names. This test ensures the next one doesn't slip in.
//
// What this test does:
//   Walks every .tsx file under apps/frontend/src and packages/ui/src,
//   finds every `<Button …size="icon"…>` opening tag (handling JSX
//   expression braces in attribute values), and asserts each one
//   either:
//     a) carries `aria-label`, `aria-labelledby`, or `title` in its
//        opening tag, OR
//     b) is preceded (within the prior 8 lines) by `<TooltipTrigger
//        asChild>`, in which case the Tooltip provides the name.
//   Test files (`.test.tsx`, `__tests__/`) are excluded.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const FRONTEND_ROOT = resolve(__dirname, "..", "..", "..", "frontend", "src");
const UI_ROOT = resolve(__dirname, "..", "..", "..", "..", "packages", "ui", "src");

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
    else if (st.isFile() && full.endsWith(".tsx")) {
      if (full.endsWith(".test.tsx")) continue;
      out.push(full);
    }
  }
  return out;
}

/** Find each `<Button …>` opening tag. Returns start index, open-tag end, tag text, 1-based line. */
function findButtonOpenings(
  text: string,
): { start: number; openEnd: number; tag: string; line: number }[] {
  const re = /<Button(?=[\s>])/g;
  const results: { start: number; openEnd: number; tag: string; line: number }[] = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    const start = m.index;
    let depth = 0;
    let end = -1;
    for (let p = start + "<Button".length; p < text.length; p++) {
      const ch = text[p];
      if (ch === "{") depth++;
      else if (ch === "}") depth--;
      else if (ch === ">" && depth === 0) {
        end = p;
        break;
      }
    }
    if (end === -1) continue;
    const tag = text.slice(start, end + 1);
    const line = text.slice(0, start).split("\n").length;
    results.push({ start, openEnd: end, tag, line });
  }
  return results;
}

function isIconButton(tag: string): boolean {
  return /\bsize=("icon"|\{["']icon["']\})/.test(tag);
}

function isSelfClosing(tag: string): boolean {
  return /\/\s*>$/.test(tag);
}

function hasAccessibleName(tag: string): boolean {
  return (
    /\baria-label[=\s]/.test(tag) || /\baria-labelledby[=\s]/.test(tag) || /\btitle=/.test(tag)
  );
}

function precededByTooltipTrigger(text: string, start: number): boolean {
  // Look back up to 8 lines for `<TooltipTrigger asChild>`.
  const before = text.slice(0, start).split("\n");
  const tail = before.slice(Math.max(0, before.length - 8)).join("\n");
  return /<TooltipTrigger\s+asChild\s*>/.test(tail);
}

/**
 * Returns true if the button body (between opening `>` and matching
 * `</Button>`) contains a child with `className` including `sr-only` —
 * the canonical visually-hidden label pattern.
 */
function hasSrOnlyChild(text: string, openEnd: number): boolean {
  let depth = 1;
  let i = openEnd + 1;
  while (i < text.length && depth > 0) {
    // Skip strings and braces lightly; we only need to track Button
    // open/close. Search for whichever comes first.
    const nextOpen = text.indexOf("<Button", i);
    const nextClose = text.indexOf("</Button", i);
    if (nextClose === -1) return false;
    if (nextOpen !== -1 && nextOpen < nextClose) {
      // Verify it's actually <Button (followed by whitespace or >)
      const after = text[nextOpen + "<Button".length];
      if (after === " " || after === "\t" || after === "\n" || after === ">") {
        depth++;
      }
      i = nextOpen + "<Button".length;
    } else {
      depth--;
      if (depth === 0) {
        const body = text.slice(openEnd + 1, nextClose);
        return /className=("[^"]*\bsr-only\b[^"]*"|\{[^}]*\bsr-only\b[^}]*\})/.test(body);
      }
      i = nextClose + "</Button".length;
    }
  }
  return false;
}

describe("icon-button accessibility discipline", () => {
  it("every <Button size='icon'> opener has aria-label, aria-labelledby, title, or a Tooltip ancestor", () => {
    const offenders: { file: string; lines: number[] }[] = [];
    const files = [...walk(FRONTEND_ROOT), ...walk(UI_ROOT)];
    for (const file of files) {
      const text = readFileSync(file, "utf8");
      const offendingLines: number[] = [];
      for (const { start, openEnd, tag, line } of findButtonOpenings(text)) {
        if (!isIconButton(tag)) continue;
        if (hasAccessibleName(tag)) continue;
        if (precededByTooltipTrigger(text, start)) continue;
        if (!isSelfClosing(tag) && hasSrOnlyChild(text, openEnd)) continue;
        offendingLines.push(line);
      }
      if (offendingLines.length > 0) offenders.push({ file, lines: offendingLines });
    }
    if (offenders.length > 0) {
      const detail = offenders.map((o) => `  ${o.file}: ${o.lines.join(",")}`).join("\n");
      throw new Error(
        `Found ${offenders.length} file(s) with <Button size="icon"> elements missing accessible names.\n` +
          `Screen readers will announce these as "button" with no clue what they do.\n` +
          `Add aria-label="..." (or wrap in <TooltipTrigger asChild>) to each.\n\n` +
          detail,
      );
    }
    expect(offenders).toEqual([]);
  });
});
