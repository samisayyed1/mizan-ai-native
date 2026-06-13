import { chromium } from "@playwright/test";
const b = await chromium.launch();
for (const w of [390, 430]) {
  const ctx = await b.newContext({ viewport: { width: w, height: 900 }, deviceScaleFactor: 3, isMobile: true });
  const p = await ctx.newPage();
  await p.goto("http://localhost:3127/", { waitUntil: "networkidle" });
  await p.waitForFunction(() => getComputedStyle(document.body).backgroundColor !== "rgba(0, 0, 0, 0)");
  await p.evaluate(() => document.fonts.ready);
  // scroll showcase into view + force Overview tab
  const sc = p.locator(".app-showcase").first();
  await sc.scrollIntoViewIfNeeded();
  await p.waitForTimeout(300);
  await sc.locator('button[aria-pressed]').first().click();
  await p.waitForTimeout(400);
  // screenshot the allocation card (2nd card with the donut)
  const card = sc.locator('div').filter({ hasText: 'Sukuks' }).last();
  await sc.screenshot({ path: `screenshots/donut-${w}.png` });
  await ctx.close();
}
await b.close();
console.log("done");
