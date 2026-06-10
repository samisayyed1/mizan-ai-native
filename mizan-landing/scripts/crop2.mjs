import { chromium } from "@playwright/test";
const b = await chromium.launch();
const p = await (await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 })).newPage();
await p.goto(process.env.BASE_URL ?? "http://localhost:3127", { waitUntil: "networkidle" });
await p.evaluate(() => document.fonts.ready);
const wl = await p.$("#waitlist");
if (wl) await wl.screenshot({ path: "screenshots/crop-waitlist.png" });
// AI section: scroll to find the "Built AI-native" heading
const ai = await p.locator("section", { hasText: "Built AI-native" }).first();
if (await ai.count()) await ai.screenshot({ path: "screenshots/crop-ai.png" });
await b.close();
console.log("done");
