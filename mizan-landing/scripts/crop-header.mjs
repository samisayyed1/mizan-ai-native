import { chromium } from "@playwright/test";
const b = await chromium.launch();
const p = await (await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 3 })).newPage();
await p.goto(process.env.BASE_URL ?? "http://localhost:3127", { waitUntil: "networkidle" });
await p.evaluate(() => document.fonts.ready);
await p.screenshot({ path: "screenshots/crop-header.png", clip: { x: 880, y: 8, width: 480, height: 60 } });
await b.close();
console.log("done");
