import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

// Same flow as ui_flow.spec.ts, but UI *and* API are both served by the
// Cloudflare worker (wrangler dev via docker/docker-compose.cloudflare-e2e.yml).
// The PDF step from ui_flow is absent: pdf endpoints are #[cfg(not(wasm32))]
// and don't exist on the worker.
let stepNo = 0;
async function shot(page: any, name: string) {
  stepNo += 1;
  const out = path.join(__dirname, "..", "captures", "cloudflare_ui_flow");
  fs.mkdirSync(out, { recursive: true });
  await page.screenshot({ path: path.join(out, stepNo + "-" + name + ".png"), fullPage: true });
}

const BACKEND = process.env.CF_BACKEND ?? "http://localhost:18787";
const UI = process.env.CF_UI ?? BACKEND;

const rand = () => Math.random().toString(36).slice(2, 10);

async function api(method: string, apiPath: string, token?: string, body?: unknown) {
  return request.fetch(`${BACKEND}/api${apiPath}`, {
    method,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    data: body ? JSON.stringify(body) : undefined,
  });
}

let request: any;

test.beforeEach(({ request: r }) => {
  request = r;
  stepNo = 0;
});

test("cf ui flow: signup → shop → goods → wallet → sell → qr capture screen", async ({ page }) => {
  test.setTimeout(120_000);
  const email = `cf-ui-owner-${rand()}@test.dev`;
  const password = "hunter2234";
  const address = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";

  await page.goto(`${UI}/?api=${encodeURIComponent(BACKEND + "/api")}`);
  await expect(page.locator("h1")).toHaveText("solsulsol");
  await shot(page, "01-login-screen-served-by-worker");

  // register + login
  await page.locator('input[placeholder="email"]').fill(email);
  await page.locator('input[placeholder="password"]').fill(password);
  await page.getByRole("button", { name: "Register" }).click();
  await expect(page.locator(".toast.success")).toContainText("registered");
  await shot(page, "02-register");
  await page.getByRole("button", { name: "Login", exact: true }).click();
  await expect(page.locator('[aria-label="new-shop"]')).toBeVisible({ timeout: 20_000 });

  // create shop
  await shot(page, "03-login-shop-select-page");
  await page.locator('[aria-label="new-shop"]').click();
  await page.locator('input[placeholder="new shop name"]').fill("CF UI Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  const shopChip = page.locator(".shop-chip", { hasText: "CF UI Cafe" });
  await expect(shopChip).toBeVisible({ timeout: 20_000 });

  // open shop panel → sell tab
  await shot(page, "04-create-shop-via-modal");
  await shopChip.click();
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator('[aria-label="tab-sell"]')).toBeVisible();
  await shot(page, "05-open-shop-panel-sell-tab");

  // goods: espresso + latte
  await page.locator('[aria-label="tab-goods"]').click();
  for (const [name, price, cost] of [["espresso", "3.5", "1.2"], ["latte", "4", "1.5"]] as const) {
    await page.locator('[aria-label="create-goods"]').click();
    await page.locator('input[placeholder="name"]').fill(name);
    await page.locator('input[placeholder="price"]').fill(price);
    await page.locator('input[placeholder="cost"]').fill(cost);
    await page.locator('input[placeholder="units"]').fill("10");
    await page.locator('[aria-label="add-goods"]').click();
    await expect(page.locator("tr", { hasText: name })).toBeVisible({ timeout: 20_000 });
  }
  await shot(page, "06-goods-created-on-d1");

  // settings → solana receive account
  await page.locator('[aria-label="tab-settings"]').click();
  await page.locator('[aria-label="section-add-account"]').click();
  await page.locator('input[placeholder="address"]').fill(address);
  await page.locator('[aria-label="add-destination"]').click();
  await expect(page.locator(".list-row", { hasText: "Solana" })).toBeVisible({ timeout: 20_000 });
  await shot(page, "07-settings-solana-receive-account");

  // sell: pick espresso, verify total
  await page.locator('[aria-label="tab-sell"]').click();
  const tile = page.locator(".goods-tile", { hasText: "espresso" });
  await expect(tile).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".page-title")).toHaveText("New sale");
  await tile.click();
  await expect(tile).toHaveClass(/picked/);
  await expect(page.locator(".total-bar")).toContainText("3.5");
  await shot(page, "08-sell-tab-espresso-picked");

  // confirm → billing QR with countdown (worker renders the solana: payload)
  await page.locator('[aria-label="new-doc"]').click();
  const qr = page.locator(".qr svg").first();
  await expect(qr).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".pay-total").first()).toHaveText(/3\.5/);
  await expect(page.locator(".countdown").first()).toHaveText(/[45]:\d{2}/);
  await shot(page, "09-confirm-billing-qr-with-countdown");
  await page.locator('[aria-label="close-modal"]').click();

  // invoice persisted — verify through the worker API (profit snapshot included)
  const login = await api("POST", "/auth/login", undefined, { email, password });
  const { token } = await login.json();
  const shops = await (await api("GET", "/shops", token)).json();
  const shopId = shops[0].shop_id;
  const billings = await (await api("GET", `/shops/${shopId}/billings`, token)).json();
  expect(billings).toHaveLength(1);
  expect(billings[0].total).toBe("3.5");
  expect(billings[0].cost).toBe("1.2");
  expect(billings[0].qr_payload).toContain("solana:");
  await shot(page, "10-invoice-persisted-verified-via-worker-api");

  // pdf_url exists in the API payload but the endpoint is absent on the worker
  expect(billings[0].pdf_url).toBeTruthy();
  const pdf = await api("GET", billings[0].pdf_url, token);
  expect(pdf.status()).toBe(404);

  // reports: revenue & profit
  await page.locator('[aria-label="tab-reports"]').click();
  await expect(page.locator(".stat-value").first()).toHaveText(/3\.5/, { timeout: 20_000 });
  await expect(page.locator(".stat-value.good")).toHaveText(/2\.3/);
  await expect(page.locator(".list-row", { hasText: "espresso" }).first()).toBeVisible();
  await shot(page, "11-reports-revenue-profit");

  // documents nav shows the sale
  await page.locator('[aria-label="tab-sell"]').click();
  await page.locator(".settings-tab", { hasText: "Documents" }).click();
  await expect(page.locator(".page-title", { hasText: "Documents" })).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".list-row", { hasText: "espresso" })).toBeVisible({ timeout: 20_000 });
  await shot(page, "12-documents-show-sale");

  // settings shows the receive account, then back to sell → stock decremented
  // (visiting another main tab first resets the sell sub-tab, like ui_flow step 13)
  await page.locator('[aria-label="tab-settings"]').click();
  await expect(page.locator(".list-row", { hasText: "Solana" })).toBeVisible({ timeout: 20_000 });
  await shot(page, "13-settings-receive-account");

  await page.locator('[aria-label="tab-sell"]').click();
  await expect(page.locator(".goods-grid")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".goods-tile", { hasText: "espresso" })).toContainText("x9");
  await shot(page, "14-done-stock-decremented");
});
