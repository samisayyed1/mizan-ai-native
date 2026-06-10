import { chromium, devices } from "@playwright/test";
const b = await chromium.launch();

async function animState(viewport, deviceOpts) {
  const ctx = await b.newContext(deviceOpts ?? { viewport });
  const p = await ctx.newPage();
  await p.goto("http://localhost:3127/", { waitUntil: "load" });
  await p.waitForFunction(() => getComputedStyle(document.body).backgroundColor !== "rgba(0, 0, 0, 0)", { timeout: 15000 });
  const r = await p.evaluate(() => {
    const get = (sel) => { const el = document.querySelector(sel); return el ? getComputedStyle(el).animationName : "MISSING"; };
    return {
      orb: get(".animate-orb"),
      sweep: get(".hero-headline-sweep"),
      cta: get(".cta-glow"),
      overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    };
  });
  await ctx.close();
  return r;
}

const mobile = await animState(null, { ...devices["iPhone 14 Pro"] });
const desktop = await animState({ width: 1440, height: 900 });
console.log("MOBILE (should be 'none' = no continuous repaint):", JSON.stringify(mobile));
console.log("DESKTOP (should have animation names):", JSON.stringify(desktop));

// capture mobile full page + hero
const ctx = await b.newContext({ ...devices["iPhone 14 Pro"] });
const p = await ctx.newPage();
await p.goto("http://localhost:3127/", { waitUntil: "networkidle" });
await p.waitForFunction(() => getComputedStyle(document.body).backgroundColor !== "rgba(0, 0, 0, 0)");
await p.evaluate(() => document.fonts.ready);
await p.waitForTimeout(400);
await p.screenshot({ path: "screenshots/m-hero.png" });
await b.close();
console.log("captured m-hero");
