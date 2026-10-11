import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { apiAuth, conn, ensureUsdc, freshWallet } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe, saveVideo } from "./helpers/ui";

test.afterEach(async ({ page }) => {
  await saveVideo(page, "ui_flow");
});

let stepNo = 0;
async function shot(page: any, dir: string, name: string) {
  stepNo += 1;
  const out = path.join(__dirname, "..", "captures", dir);
  fs.mkdirSync(out, { recursive: true });
  await page.screenshot({ path: path.join(out, stepNo + "-" + name + ".png"), fullPage: true });
}


const BACKEND = process.env.BACKEND ?? "http://localhost:18080";
const UI = process.env.UI ?? "http://localhost:8081";

const ADDRESS = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";

async function api(method: string, apiPath: string, token?: string, body?: unknown) {
  return request.fetch(`${BACKEND}${apiPath}`, {
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

const FLOW = "ui: wallet signin → subscribe → shop → goods → wallet → sell → qr capture screen";

test(`${FLOW} — desktop`, async ({ page }) => {
  await runFlow(page);
});

test(`${FLOW} — mobile`, async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await runFlow(page, "ui_flow_mobile");
});



async function runFlow(page: any, dir = "ui_flow") {
  test.setTimeout(90_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  // 01: login screen (wallet connect card)
  await gotoUi(page);
  await shot(page, dir, "01-login-screen-icon-brand");

  // 02: connect mock Phantom → sign in → dashboard
  await shot(page, dir, "02-connect-wallet-sign-in");
  await uiSignIn(page);

  // 03: + New shop — creation is free, form opens directly
  await page.locator('[aria-label="new-shop"]').click();
  await expect(page.locator('input[placeholder="new shop name"]')).toBeVisible({ timeout: 20_000 });
  await shot(page, dir, "03-create-shop-form");

  // 04: create the shop
  await page.locator('input[placeholder="new shop name"]').fill("UI Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  // 05: unpaid — gate blocks before the shop; pay to enter
  await uiSubscribe(page);
  await expect(page.locator('[aria-label="tab-sell"]')).toBeVisible();

  // 06: goods tab → create goods modal with price, cost, units
  await shot(page, dir, "06-goods-tab-create-goods-modal-with-price-");
  await page.locator('[aria-label="tab-goods"]').click();
  await page.locator('[aria-label="create-goods"]').click();
  await page.locator('input[placeholder="name"]').fill("espresso");
  await page.locator('input[placeholder="price"]').fill("3.5");
  await page.locator('input[placeholder="cost"]').fill("1.2");
  await page.locator('input[placeholder="units"]').fill("10");
  await page.locator('[aria-label="goods-type"]').selectOption("food");
  await shot(page, dir, "06b-goods-form-type-dropdown-all-icons-colors");
  await page.locator('[aria-label="add-goods"]').click();
  const goodsRow = page.locator("tr", { hasText: "espresso" });
  await expect(goodsRow).toBeVisible({ timeout: 15_000 });
  await expect(goodsRow).toContainText("x10");

  // 07: settings → payments group → add solana receive account
  await shot(page, dir, "07-settings-payments-group-add-solana-recei");
  await page.locator('[aria-label="tab-settings"]').click();
  await page.locator('[aria-label="section-add-account"]').click();
  await page.locator('input[placeholder="address"]').fill(ADDRESS);
  await page.locator('[aria-label="add-destination"]').click();
  // success lands back on the Receive accounts section
  await expect(page.locator(".list-row", { hasText: "Solana" })).toBeVisible({ timeout: 15_000 });

  // 08: sell tab → pick goods tile, doc draft, destination chip
  await shot(page, dir, "08-sell-tab-pick-goods-tile-doc-draft-desti");
  await page.locator('[aria-label="tab-sell"]').click();
  const tile = page.locator(".goods-tile", { hasText: "espresso" });
  await expect(tile).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".page-title")).toHaveText("New sale");
  await tile.click();
  await expect(tile).toHaveClass(/picked/);
  await expect(page.locator(".total-bar")).toContainText("3.5");

  // 08b: hold the draft, start a new one, then return to the held bill
  await shot(page, dir, "08b-hold-the-draft-start-a-new-one-then-retu");
  const latte = page.locator(".goods-tile", { hasText: "latte" });
  await page.locator('[aria-label="tab-goods"]').click();
  await page.locator('[aria-label="create-goods"]').click();
  await page.locator('input[placeholder="name"]').fill("latte");
  await page.locator('input[placeholder="price"]').fill("4");
  await page.locator('input[placeholder="cost"]').fill("1.5");
  await page.locator('input[placeholder="units"]').fill("10");
  await page.locator('[aria-label="add-goods"]').click();
  await expect(page.locator("tr", { hasText: "latte" })).toBeVisible({ timeout: 15_000 });
  await page.locator('[aria-label="tab-sell"]').click();
  await expect(latte).toBeVisible({ timeout: 15_000 });
  await latte.click();
  await page.locator('[aria-label="hold-bill"]').click();
  await expect(page.locator('[aria-label="held-bill-0"]')).toContainText("Bill 1");
  await expect(page.locator(".total-bar")).toHaveCount(0);
  await tile.click();
  await page.locator('[aria-label="held-bill-0"]').click();
  await expect(latte).toHaveClass(/picked/);
  await expect(tile).not.toHaveClass(/picked/);
  // keep only espresso for the rest of the flow
  await tile.click();
  await latte.click();
  await expect(page.locator(".total-bar")).toContainText("3.5");

  // 09: confirm → choose payment method → billing QR with countdown
  await shot(page, dir, "09-confirm-billing-qr-with-countdown");
  await page.locator('[aria-label="new-doc"]').click();
  await expect(page.locator('[aria-label="pay-method"]').first()).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(300);
  await shot(page, dir, "09a-choose-payment-method-sheet");
  await page.locator('[aria-label="pay-method"]').first().click();
  const qr = page.locator(".qr svg").first();
  await expect(qr).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".pay-total").first()).toHaveText(/3\.5/);
  await expect(page.locator(".countdown").first()).toHaveText(/[45]:\d{2}/);
  await page.locator('[aria-label="close-modal"]').click();

  // 10: invoice persisted — verify through the API (profit snapshot included)
  await shot(page, dir, "10-invoice-persisted-verify-through-the-api");
  const { token } = await apiAuth(api, wallet);
  const shops = await (await api("GET", "/shops", token)).json();
  const shopId = shops[0].shop_id;
  const billings = await (await api("GET", `/shops/${shopId}/billings`, token)).json();
  expect(billings).toHaveLength(1);
  expect(billings[0].total).toBe("3.5");
  expect(billings[0].cost).toBe("1.2");
  expect(billings[0].qr_payload).toBeTruthy();

  // 10b: capture the billing PDF served by the backend
  const pdf = await api("GET", billings[0].pdf_url, token);
  expect(pdf.status()).toBe(200);
  const pdfBuf = Buffer.from(await pdf.body());
  expect(pdfBuf.subarray(0, 5).toString()).toBe("%PDF-");
  console.log(`[pdf] ${pdfBuf.length} bytes`);

  // 11: reports tab → revenue & profit from the sale document
  await shot(page, dir, "11-reports-tab-revenue-profit-from-the-sale");
  await page.locator('[aria-label="tab-reports"]').click();
  await expect(page.locator(".stat-value").first()).toHaveText(/3\.5/, { timeout: 15_000 });
  await expect(page.locator(".stat-value.good")).toHaveText(/2\.3/);
  await expect(page.locator(".list-row", { hasText: "espresso" }).first()).toBeVisible();

  // 12: sell tab → documents nav shows the sale document
  await shot(page, dir, "12-sell-tab-documents-nav-shows-the-sale-do");
  await page.locator('[aria-label="tab-sell"]').click();
  await page.locator(".settings-tab", { hasText: "Documents" }).click();
  await expect(page.locator(".page-title", { hasText: "Documents" })).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".list-row", { hasText: "espresso" })).toBeVisible({ timeout: 15_000 });

  // 13: settings tab shows receive account + stock counted down
  await shot(page, dir, "13-settings-tab-shows-receive-account-stock");
  await page.locator('[aria-label="tab-settings"]').click();
  await expect(page.locator(".list-row", { hasText: "Solana" })).toBeVisible({ timeout: 15_000 });

  // 14: done → back to a fresh sale page, stock decremented
  await shot(page, dir, "14-done-back-to-a-fresh-sale-page-stock-dec");
  await page.locator('[aria-label="tab-sell"]').click();
  await expect(page.locator(".goods-grid")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".goods-tile", { hasText: "espresso" })).toContainText("x9");
}
