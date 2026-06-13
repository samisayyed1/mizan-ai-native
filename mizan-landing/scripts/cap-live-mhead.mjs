import { chromium, devices } from "@playwright/test";
const b = await chromium.launch();
const p = await (await b.newContext({ ...devices["iPhone 14 Pro"] })).newPage();
await p.goto("https://getmizan.net/", { waitUntil: "networkidle" });
await p.evaluate(() => document.fonts.ready);
await p.waitForTimeout(500);
await p.screenshot({ path: "screenshots/live-mhead.png", clip: { x: 0, y: 0, width: 393, height: 320 } });
const m = await p.evaluate(() => ({
  orb: (()=>{const e=document.querySelector('.animate-orb');return e?getComputedStyle(e).animationName:'?';})(),
  overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
}));
console.log("LIVE mobile:", JSON.stringify(m));
await b.close();
