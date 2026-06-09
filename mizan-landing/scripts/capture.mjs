// One-shot screenshot script — used for manual verification, not CI.
// CI uses tests/visual.spec.ts.
import { chromium, devices } from "@playwright/test";
import path from "node:path";
import fs from "node:fs/promises";

const BASE = process.env.BASE_URL ?? "http://localhost:3127";
const OUT_DIR = path.resolve("screenshots");

const TARGETS = [
  { name: "desktop-1440", viewport: { width: 1440, height: 900 } },
  { name: "mobile-393", ...devices["iPhone 14 Pro"] },
];

await fs.mkdir(OUT_DIR, { recursive: true });

const browser = await chromium.launch();
for (const t of TARGETS) {
  const context = await browser.newContext({ ...t, deviceScaleFactor: 2 });
  const page = await context.newPage();
  await page.goto(BASE, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({
    path: path.join(OUT_DIR, `${t.name}-full.png`),
    fullPage: true,
  });
  await page.screenshot({
    path: path.join(OUT_DIR, `${t.name}-fold.png`),
  });
  console.log(`captured ${t.name}`);
  await context.close();
}
await browser.close();
