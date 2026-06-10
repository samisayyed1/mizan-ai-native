import { chromium, devices } from "@playwright/test";
const b = await chromium.launch();
const ctx = await b.newContext({ ...devices["iPhone 14 Pro"] });
const p = await ctx.newPage();
await p.goto("http://localhost:3127/", { waitUntil: "networkidle" });
await p.waitForFunction(() => getComputedStyle(document.body).backgroundColor !== "rgba(0, 0, 0, 0)");
await p.evaluate(() => document.fonts.ready);
await p.waitForTimeout(400);
const shots = [
  ["a1-hero", "header"],
  ["a2-trust", 'section[aria-label="Trust signals"]'],
  ["a3-problem", 'section:has-text("visibility problem")'],
  ["a4-ai", 'section:has-text("Built AI-native")'],
  ["a5-faq", "#faq"],
  ["a6-waitlist", "#waitlist"],
  ["a7-founder", 'section:has-text("Why we built Mizan")'],
  ["a8-footer", "footer"],
];
for (const [name, sel] of shots) {
  try {
    const el = p.locator(sel).first();
    await el.scrollIntoViewIfNeeded();
    await p.waitForTimeout(250);
    await el.screenshot({ path: `screenshots/${name}.png` });
  } catch(e) { console.log("skip", name, e.message.slice(0,60)); }
}
console.log("done");
await b.close();
