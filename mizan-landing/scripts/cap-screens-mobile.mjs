import { chromium } from "@playwright/test";
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 390, height: 1100 }, deviceScaleFactor: 3, isMobile: true });
const p = await ctx.newPage();
await p.goto("http://localhost:3127/", { waitUntil: "networkidle" });
await p.evaluate(() => document.fonts.ready);
const sc = p.locator(".app-showcase").first();
await sc.scrollIntoViewIfNeeded();
await p.waitForTimeout(300);
const tabs = ["overview","goals","ai","alerts","news","accounts"];
let issues = [];
for (let i=0;i<tabs.length;i++){
  await sc.locator('button[aria-pressed]').nth(i).click();
  await p.waitForTimeout(450);
  await sc.screenshot({ path: `screenshots/ms-${tabs[i]}.png` });
  // atomic overflow check inside the screen
  const overflow = await p.evaluate(() => {
    const root = document.querySelector('.app-showcase');
    let bad = [];
    root.querySelectorAll('*').forEach(el => {
      if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflow === 'visible') {
        const t = (el.textContent||'').trim().slice(0,24);
        if (t) bad.push(t);
      }
    });
    return [...new Set(bad)].slice(0,6);
  });
  if (overflow.length) issues.push(`${tabs[i]}: ${overflow.join(' | ')}`);
}
console.log(issues.length ? "OVERFLOWS:\n"+issues.join("\n") : "no text overflow detected");
await b.close();
