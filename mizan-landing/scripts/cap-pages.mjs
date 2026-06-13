import { chromium } from "@playwright/test";
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 1280, height: 1400 }, deviceScaleFactor: 2 });
const p = await ctx.newPage();
for (const [name,url] of [["privacy","/privacy"],["security","/security"]]) {
  await p.goto("http://localhost:3127"+url, { waitUntil: "networkidle" });
  await p.evaluate(() => document.fonts.ready);
  await p.screenshot({ path: `screenshots/page-${name}.png` });
}
// FAQ section on home
await p.goto("http://localhost:3127/#faq", { waitUntil: "networkidle" });
await p.evaluate(() => document.fonts.ready);
const faq = p.locator("#faq");
await faq.scrollIntoViewIfNeeded();
// open first two details for the shot
await p.evaluate(() => { document.querySelectorAll('#faq details').forEach((d,i)=>{ if(i<2) d.open=true; }); });
await p.waitForTimeout(300);
await faq.screenshot({ path: "screenshots/page-faq.png" });
await b.close();
console.log("done");
