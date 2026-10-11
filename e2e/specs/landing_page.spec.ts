// Landing page capture: serves demo/landing-page/dist statically and
// screenshots the single-page layout on desktop + mobile. No backend needed.
import { test, expect } from "@playwright/test";
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { saveVideo } from "./helpers/ui";

const DIST = path.join(__dirname, "..", "..", "demo", "landing-page", "dist");
const LANDING = process.env.LANDING_URL; // optional: external trunk serve
const PORT = 18099;

const MIME: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".css": "text/css",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};

let server: http.Server | undefined;
let baseUrl: string;

test.beforeAll(async () => {
  if (LANDING) {
    baseUrl = LANDING;
    return;
  }
  if (!fs.existsSync(path.join(DIST, "index.html"))) {
    throw new Error(`landing page not built: run 'trunk build --release' in demo/landing-page`);
  }
  server = http.createServer((req, res) => {
    const url = (req.url ?? "/").split("?")[0];
    const file = path.join(DIST, url === "/" ? "index.html" : url);
    fs.readFile(file, (err, data) => {
      if (err) {
        res.writeHead(404).end();
        return;
      }
      res.writeHead(200, { "Content-Type": MIME[path.extname(file)] ?? "application/octet-stream" });
      res.end(data);
    });
  });
  await new Promise<void>((ok) => server!.listen(PORT, ok));
  baseUrl = `http://localhost:${PORT}`;
});

test.afterAll(async () => {
  await new Promise<void>((ok) => (server ? server.close(() => ok()) : ok()));
});

let stepNo = 0;
async function shot(page: any, name: string) {
  stepNo += 1;
  const out = path.join(__dirname, "..", "captures", "landing_page");
  fs.mkdirSync(out, { recursive: true });
  await page.screenshot({ path: path.join(out, stepNo + "-" + name + ".png"), fullPage: true });
}

test.afterEach(async ({ page }) => {
  await saveVideo(page, "landing_page");
});

const FLOW = "landing one-pager: hero + features + chains captured";

test(`${FLOW} — desktop`, async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto(baseUrl);
  await expect(page.locator("h1")).toContainText("Point of Sale that settles on Solana");
  await expect(page.locator(".grid .card")).toHaveCount(6);
  await expect(page.locator(".roadmap-title")).toBeVisible();
  await expect(page.locator(".card.coming")).toBeVisible();
  const cta = page.locator("a.btn");
  await expect(cta).toHaveCount(1);
  await expect(cta).toHaveAttribute("href", "http://localhost:8081");
  await shot(page, "onepager");
});

test(`${FLOW} — mobile`, async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(baseUrl);
  await expect(page.locator("h1")).toContainText("Point of Sale that settles on Solana");
  await expect(page.locator(".grid .card")).toHaveCount(6);
  await expect(page.locator(".roadmap-title")).toBeVisible();
  await shot(page, "onepager_mobile");
});
