import { chromium } from "@playwright/test";
const REF = "hGnfBSph"; // Feroz's
const b = await chromium.launch();
const p = await (await b.newContext()).newPage();
console.log("1. clicking invite link /i/" + REF);
await p.goto("http://localhost:3127/i/" + REF, { waitUntil: "networkidle" });
console.log("   landed at:", p.url());
console.log("   has ?ref in URL:", p.url().includes("ref=" + REF));
// fill the form
const email = `invite-test+${Date.now()}@getmizan.net`;
await p.getByLabel(/email/i).first().fill(email);
const btn = p.getByRole("button", { name: /reserve my spot/i });
await btn.scrollIntoViewIfNeeded();
await btn.click();
// wait for confirmation
await p.waitForSelector("text=/you're on the list/i", { timeout: 8000 });
console.log("2. submitted, confirmation shown for", email);
await b.close();
console.log("done");
