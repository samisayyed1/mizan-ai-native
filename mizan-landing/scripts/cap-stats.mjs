import { chromium, devices } from "@playwright/test";
const TOKEN = "mizan-insider-e21c63544b9b0c78cc9f874a";
const b = await chromium.launch();
// dashboard desktop
const d = await b.newContext({ viewport: { width: 1100, height: 1000 }, deviceScaleFactor: 2 });
const p1 = await d.newPage();
await p1.goto("http://localhost:3127/stats?key=" + TOKEN, { waitUntil: "networkidle" });
await p1.evaluate(() => document.fonts.ready);
await p1.waitForTimeout(1000);
await p1.screenshot({ path: "screenshots/stats-desktop.png", fullPage: true });
await d.close();
// dashboard mobile
const m = await b.newContext({ ...devices["iPhone 14 Pro"] });
const p2 = await m.newPage();
await p2.goto("http://localhost:3127/stats?key=" + TOKEN, { waitUntil: "networkidle" });
await p2.evaluate(() => document.fonts.ready);
await p2.waitForTimeout(1000);
await p2.screenshot({ path: "screenshots/stats-mobile.png", fullPage: true });
await p2.close();
// lock screen
const l = await b.newContext({ viewport: { width: 1100, height: 800 }, deviceScaleFactor: 2 });
const p3 = await l.newPage();
await p3.goto("http://localhost:3127/stats", { waitUntil: "networkidle" });
await p3.evaluate(() => document.fonts.ready);
await p3.waitForTimeout(400);
await p3.screenshot({ path: "screenshots/stats-lock.png" });
await b.close();
console.log("done");
