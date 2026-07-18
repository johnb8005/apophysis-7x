// End-to-end smoke check: loads the app in a real browser, waits for the
// renderer to produce a frame, and reports console/page errors.
//
// Usage: bun scripts/smoke.mjs [url] [outfile]

import { chromium } from "playwright";

const url = process.argv[2] ?? "http://localhost:4173/";
const out = process.argv[3] ?? "smoke.png";

// Use the system Chrome rather than Playwright's pinned download — it is
// already present, and this keeps CI from fetching a browser it doesn't need.
const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH ?? "/usr/bin/google-chrome",
});
const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });

const errors = [];
page.on("console", (m) => {
  const t = m.type();
  if (t === "error" || t === "warning") errors.push(`[${t}] ${m.text()}`);
});
page.on("pageerror", (e) => errors.push(`[pageerror] ${e.message}`));
page.on("requestfailed", (r) =>
  errors.push(`[requestfailed] ${r.url()} — ${r.failure()?.errorText}`),
);

await page.goto(url, { waitUntil: "networkidle" });

let status = "ok";
try {
  // The canvas only mounts once the first frame arrives from the worker.
  await page.waitForSelector("canvas", { timeout: 45_000 });
  // Let the frame paint before capturing.
  await page.waitForFunction(
    () => {
      const c = document.querySelector("canvas");
      return c instanceof HTMLCanvasElement && c.width > 0 && c.height > 0;
    },
    { timeout: 10_000 },
  );
} catch (e) {
  status = `FAILED: ${e.message.split("\n")[0]}`;
}

// Sample the canvas to confirm it actually contains an image, not just blank.
const stats = await page.evaluate(() => {
  const c = document.querySelector("canvas");
  if (!(c instanceof HTMLCanvasElement)) return null;
  const ctx = c.getContext("2d");
  if (!ctx) return null;
  const { data } = ctx.getImageData(0, 0, c.width, c.height);
  let lit = 0;
  for (let i = 0; i < data.length; i += 4) {
    if (data[i] > 8 || data[i + 1] > 8 || data[i + 2] > 8) lit++;
  }
  return { width: c.width, height: c.height, litPct: (100 * lit) / (c.width * c.height) };
});

await page.screenshot({ path: out });
await browser.close();

console.log(`status: ${status}`);
console.log(`canvas: ${stats ? `${stats.width}x${stats.height}, ${stats.litPct.toFixed(1)}% lit` : "none"}`);
if (errors.length) {
  console.log(`\n${errors.length} console/page issue(s):`);
  for (const e of errors.slice(0, 15)) console.log("  " + e);
} else {
  console.log("no console errors");
}

process.exit(status === "ok" && stats && stats.litPct > 0.5 ? 0 : 1);
