import { chromium, devices } from "@playwright/test";
const b = await chromium.launch();
const p = await (await b.newContext({ ...devices["iPhone 14 Pro"] })).newPage();
await p.goto("http://localhost:3127/", { waitUntil: "networkidle" });
await p.evaluate(() => document.fonts.ready);

async function clickAndCheck(label, targetId) {
  await p.evaluate(() => window.scrollTo(0,0));
  await p.waitForTimeout(300);
  await p.getByRole("link", { name: new RegExp(label, "i") }).first().click();
  await p.waitForTimeout(1200); // let smooth scroll finish
  const r = await p.evaluate((id) => {
    const el = document.getElementById(id);
    const rect = el.getBoundingClientRect();
    // find the RevealOnScroll wrapper inside and read its opacity
    const op = getComputedStyle(el).opacity;
    return { top: Math.round(rect.top), scrollY: Math.round(window.scrollY), opacity: op };
  }, targetId);
  console.log(`${label} -> #${targetId}: landed top=${r.top}px scrollY=${r.scrollY} opacity=${r.opacity}`);
}
await clickAndCheck("See how it works", "product");
await clickAndCheck("Reserve my spot", "waitlist");
await b.close();
