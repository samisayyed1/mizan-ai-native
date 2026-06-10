import { chromium, devices } from "@playwright/test";
const b = await chromium.launch();
const p = await (await b.newContext({ ...devices["iPhone 14 Pro"] })).newPage();
await p.goto(process.env.BASE_URL ?? "http://localhost:3127", { waitUntil: "load" });
// Wait until the dark theme bg is actually applied (CSS parsed + applied).
await p.waitForFunction(() => {
  const bg = getComputedStyle(document.body).backgroundColor;
  return bg && bg !== "rgba(0, 0, 0, 0)" && bg !== "rgb(255, 255, 255)";
}, { timeout: 15000 });
await p.evaluate(() => document.fonts.ready);
await p.waitForTimeout(500);
const sc = p.locator(".app-showcase").first();
await sc.scrollIntoViewIfNeeded();
await p.waitForTimeout(400);
await sc.screenshot({ path: "screenshots/crop-mobile-showcase.png" });
const overflow = await p.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
console.log("horizontal overflow px:", overflow);
await b.close();
