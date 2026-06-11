import { chromium } from "@playwright/test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
const home = os.homedir();
const svg = fs.readFileSync(path.join(home, "Downloads/mizan-logo-master.svg"), "utf8");
const b = await chromium.launch();
// Square PNG (the upload file)
const sq = await b.newContext({ viewport: { width: 1080, height: 1080 }, deviceScaleFactor: 1 });
const p1 = await sq.newPage();
await p1.setContent(`<!doctype html><html><body style="margin:0">${svg}</body></html>`);
await p1.waitForTimeout(200);
await p1.screenshot({ path: path.join(home, "Downloads/mizan-instagram-pfp-1080.png") });
// Circle preview (how IG crops it)
const cp = await b.newContext({ viewport: { width: 1080, height: 1080 }, deviceScaleFactor: 1 });
const p2 = await cp.newPage();
await p2.setContent(`<!doctype html><html><body style="margin:0;background:#000"><div style="width:1080px;height:1080px;border-radius:50%;overflow:hidden">${svg}</div></body></html>`);
await p2.waitForTimeout(200);
await p2.screenshot({ path: path.join(home, "Downloads/mizan-instagram-pfp-circle-preview.png"), omitBackground: true });
await b.close();
console.log("rendered");
