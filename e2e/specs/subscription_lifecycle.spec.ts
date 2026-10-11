import { test, expect } from "@playwright/test";
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import {
  apiAuth,
  conn,
  ensureUsdc,
  freshWallet,
  paySubscription,
  paySubscriptionAmount,
} from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, saveVideo } from "./helpers/ui";

// Subscription lifecycle, end to end with captures + video at every step:
//   before gate → subscribe monthly → active detail in Settings →
//   renew before expiry (monthly, expiry stacks) → renew yearly ($10) →
//   scan barcode by typing the number → landing CTA → POS.

const SUB_VERIFYING = "Verifying payment…";

const BACKEND = process.env.BACKEND ?? "http://host.docker.internal:18080";
const UI = process.env.UI ?? "http://127.0.0.1:8081";
const EAN13 = "4006381333931";
const YEARLY_USDC = 10_000_000n; // $10, 6 decimals

const DIST = path.join(__dirname, "..", "..", "demo", "landing-page", "dist");
const MIME: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".css": "text/css",
};

function api(method: string, url: string, token?: string, body?: unknown) {
  return request.fetch(`${BACKEND}${url}`, {
    method,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    data: body ? JSON.stringify(body) : undefined,
  });
}

async function expiresAt(token: string): Promise<number> {
  const res = await api("GET", "/subscriptions/current", token);
  expect(res.status()).toBe(200);
  return new Date((await res.json()).expires_at).getTime();
}

let request: any;
test.beforeEach(({ request: r }) => {
  request = r;
});

let shotNo = 0;
async function shot(page: any, name: string) {
  shotNo += 1;
  const out = path.join(__dirname, "..", "captures", "subscription_lifecycle");
  fs.mkdirSync(out, { recursive: true });
  await page.screenshot({
    path: path.join(out, `${String(shotNo).padStart(2, "0")}-${name}.png`),
    fullPage: true,
  });
}

/** Inside the open SubscriptionView: subscribe/renew → pay on-chain → verify. */
async function payAndVerify(page: any, minorUsdc: bigint) {
  await page.locator('[aria-label="subscribe"]').click();
  const sigInput = page.locator(
    'input[placeholder="paste transaction signature after paying"]',
  );
  await expect(sigInput).toBeVisible({ timeout: 15_000 });
  const signature =
    minorUsdc === 1_000_000n
      ? await paySubscription(conn())
      : await paySubscriptionAmount(conn(), minorUsdc);
  await sigInput.fill(signature);
  await page.getByRole("button", { name: "Verify payment" }).click();
  // Success toasts linger, so sync on the pay state instead: verification is
  // done once "Verifying payment…" is gone.
  await expect(page.getByText(SUB_VERIFYING)).toBeHidden({ timeout: 30_000 });
  await expect(page.locator(".toast.success")).toContainText("Payment confirmed", {
    timeout: 30_000,
  });
}

test.afterEach(async ({ page }) => {
  await saveVideo(page, "subscription_lifecycle");
});

test("subscription lifecycle: gate → monthly → renew before expire → yearly → scan → landing", async ({
  page,
  context,
}) => {
  test.setTimeout(240_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  // ---- BEFORE: no subscription — create free, enter, gate shows inside ---
  await gotoUi(page);
  await uiSignIn(page);
  await page.locator('[aria-label="new-shop"]').click();
  await expect(page.locator('input[placeholder="new shop name"]').first()).toBeVisible({
    timeout: 20_000,
  });
  await page.locator('input[placeholder="new shop name"]').fill("Lifecycle Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  // Created → unpaid: the gate blocks BEFORE the shop (no entry).
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required", {
    timeout: 15_000,
  });
  await expect(page.locator(".tabbar")).toHaveCount(0);
  await shot(page, "before-subscription-gate-before-shop");

  // ---- Subscribe MONTHLY through the gate → the pending shop opens ---------
  await payAndVerify(page, 1_000_000n);
  await expect(page.locator(".modal-overlay")).toHaveCount(0, { timeout: 20_000 });
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await shot(page, "after-monthly-active-inside-shop");

  // ---- Settings → Subscription -------------------------------------------
  await page.locator('[aria-label="tab-settings"]').click();
  await page.locator('[aria-label="group-subscription"]').click();
  await expect(page.locator('[aria-label="subscribe"]')).toBeVisible({ timeout: 15_000 });
  // Wait for the view's own subscription fetch: status + Active until line.
  await expect(page.locator(".card").first()).toContainText("Active until", { timeout: 15_000 });
  await shot(page, "settings-subscription-active-detail");

  // ---- RENEW BEFORE EXPIRE (monthly): expiry stacks +~1 month ------------
  const token = (await apiAuth(api, wallet)).token;
  const beforeRenew = await expiresAt(token);
  await page.locator('[aria-label="subscribe"]').click();
  await payAndVerify(page, 1_000_000n);
  const afterMonthly = await expiresAt(token);
  const monthDays = (afterMonthly - beforeRenew) / 86_400_000;
  expect(monthDays).toBeGreaterThan(27);
  expect(monthDays).toBeLessThan(32);
  await shot(page, "renewed-before-expire-monthly");

  // ---- YEARLY $10: expiry stacks +~12 months ------------------------------
  await page.locator('[aria-label="plan-yearly"]').click();
  await expect(page.locator(".pay-total").first()).toContainText("$10/year");
  await page.locator('[aria-label="subscribe"]').click();
  await payAndVerify(page, YEARLY_USDC);
  const afterYearly = await expiresAt(token);
  const yearDays = (afterYearly - afterMonthly) / 86_400_000;
  expect(yearDays).toBeGreaterThan(330);
  expect(yearDays).toBeLessThan(370);
  await shot(page, "renewed-yearly-active-until");

  // ---- SCAN: type the barcode number on the sell tab ----------------------
  // Goods with a barcode (created via API to keep the UI steps focused).
  const shops = await (await api("GET", "/shops", token)).json();
  const shopId = shops.find((s: any) => s.name === "Lifecycle Cafe").shop_id;
  expect(
    (
      await api("POST", `/shops/${shopId}/goods`, token, {
        shop_id: shopId,
        name: "bar latte",
        price: "3.5",
        barcode: EAN13,
      })
    ).status(),
  ).toBe(200);

  await page.locator('[aria-label="tab-goods"]').click();
  await page.locator('[aria-label="tab-sell"]').click();
  const scanInput = page.locator('input[placeholder="scan or type barcode, Enter to add"]');
  await expect(scanInput).toBeVisible({ timeout: 15_000 });
  await scanInput.click();
  await scanInput.pressSequentially(EAN13);
  await scanInput.press("Enter");
  await expect(page.locator(".goods-tile", { hasText: "bar latte" })).toHaveClass(/picked/, {
    timeout: 15_000,
  });
  await shot(page, "scan-barcode-typed-picked");

  // ---- LANDING → click the app CTA → POS ----------------------------------
  let server: http.Server | undefined;
  let landingUrl: string;
  if (process.env.LANDING_URL) {
    landingUrl = process.env.LANDING_URL;
  } else {
    test.skip(
      !fs.existsSync(path.join(DIST, "index.html")),
      "landing page not built: trunk build in demo/landing-page",
    );
    server = http.createServer((req, res) => {
      const url = (req.url ?? "/").split("?")[0];
      const file = path.join(DIST, url === "/" ? "index.html" : url);
      fs.readFile(file, (err, data) => {
        if (err) {
          res.writeHead(404).end();
          return;
        }
        res.writeHead(200, { "Content-Type": MIME[path.extname(file)] ?? "text/html" });
        res.end(data);
      });
    });
    const port = 18099;
    await new Promise<void>((ok) => server!.listen(port, ok));
    landingUrl = `http://localhost:${port}`;
  }
  const landing = await context.newPage();
  // The landing CTA href is the public POS URL (localhost:8081); inside docker
  // the POS lives on host.docker.internal — translate so the click navigates.
  const uiHost = new URL(UI).host;
  await landing.route("http://localhost:8081/**", async (route: any) => {
    const target = route.request().url().replace("localhost:8081", uiHost);
    await route.fulfill({ response: await route.fetch({ url: target }) });
  });
  await landing.goto(landingUrl);
  await expect(landing.locator("a.btn")).toBeVisible({ timeout: 15_000 });
  await shot(landing, "landing-page");
  await landing.locator("a.btn").click();
  await landing.waitForURL((u: URL) => u.port === "8081", { timeout: 30_000 });
  await expect(landing.locator("h1")).toHaveText("solsulsol", { timeout: 30_000 });
  await shot(landing, "landing-cta-to-pos");
  await landing.close();
  await server?.close();
});
