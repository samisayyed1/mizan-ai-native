import { chromium } from "@playwright/test";
const b = await chromium.launch();
// desktop
const d = await b.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 2 });
const p1 = await d.newPage();
await p1.goto("http://localhost:3127/", { waitUntil: "networkidle" });
await p1.evaluate(()=>document.fonts.ready);
const sc1 = p1.locator(".app-showcase").first();
await sc1.scrollIntoViewIfNeeded(); await p1.waitForTimeout(300);
await sc1.locator('button[aria-pressed]').first().click(); await p1.waitForTimeout(400);
// crop the allocation card (donut)
const donutCard = sc1.locator('div').filter({ hasText: 'Sukuks' }).last();
await donutCard.screenshot({ path: "screenshots/desk-donut.png" });
await d.close();
// mobile
const m = await b.newContext({ viewport: { width: 390, height: 900 }, deviceScaleFactor: 3, isMobile: true });
const p2 = await m.newPage();
await p2.goto("http://localhost:3127/", { waitUntil: "networkidle" });
await p2.evaluate(()=>document.fonts.ready);
const sc2 = p2.locator(".app-showcase").first();
await sc2.scrollIntoViewIfNeeded(); await p2.waitForTimeout(300);
await sc2.locator('button[aria-pressed]').first().click(); await p2.waitForTimeout(400);
const donutCard2 = sc2.locator('div').filter({ hasText: 'Sukuks' }).last();
await donutCard2.screenshot({ path: "screenshots/mob-donut.png" });
await b.close();
console.log("done");
