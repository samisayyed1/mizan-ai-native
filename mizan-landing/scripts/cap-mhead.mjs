import { chromium, devices } from "@playwright/test";
const b = await chromium.launch();
const p = await (await b.newContext({ ...devices["iPhone 14 Pro"] })).newPage();
await p.goto("http://localhost:3127/", { waitUntil: "networkidle" });
await p.waitForFunction(() => getComputedStyle(document.body).backgroundColor !== "rgba(0, 0, 0, 0)");
await p.evaluate(() => document.fonts.ready);
await p.waitForTimeout(300);
await p.screenshot({ path: "screenshots/m-head.png", clip: { x: 0, y: 0, width: 393, height: 90 } });
await b.close();
