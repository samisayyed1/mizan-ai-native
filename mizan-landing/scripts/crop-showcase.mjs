import { chromium } from "@playwright/test";
const b = await chromium.launch();
const p = await (await b.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 2 })).newPage();
await p.goto(process.env.BASE_URL ?? "http://localhost:3127", { waitUntil: "networkidle" });
await p.evaluate(() => document.fonts.ready);
const showcase = p.locator(".app-showcase").first();
await showcase.scrollIntoViewIfNeeded();
await p.waitForTimeout(400);
const tabs = ["overview","goals","ai","alerts","news","accounts"];
for (let i = 0; i < tabs.length; i++) {
  // click each tab button by index
  const btns = showcase.locator("button[aria-pressed]");
  await btns.nth(i).click();
  await p.waitForTimeout(500);
  await showcase.screenshot({ path: `screenshots/showcase-${tabs[i]}.png` });
}
await b.close();
console.log("done");
