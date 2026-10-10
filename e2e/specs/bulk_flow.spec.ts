import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { apiAuth, conn, ensureUsdc, freshWallet } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe, saveVideo } from "./helpers/ui";

test.afterEach(async ({ page }) => {
  await saveVideo(page, "bulk_flow");
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
const GOODS_UI_COUNT = 30; // every kind added through the UI form
const SCAN_START = 15; // item-015..item-029 carry barcodes (disjoint from manual clicks)
const SCAN_COUNT = 15;
const MANUAL_CLICK_COUNT = 15;
const PRICE = "2";
const VAT_RATE = "10";
const VAT_MULTIPLIER = 1 + Number(VAT_RATE) / 100;
const vat_amount = (n: number) => (n * Number(VAT_RATE)) / 100;
const STOCK_UNITS = 1000;
const SOLD_UNITS = STOCK_UNITS - 1; // one of each kind is sold

const rand = () => Math.random().toString(36).slice(2, 10);

test.setTimeout(240_000);

let request: any;

test.beforeEach(({ request: r }) => {
  request = r;
  stepNo = 0;
});

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

/** EAN-13 check digit for a 12-digit base. */
function ean13(base12: string): string {
  let sum = 0;
  for (let i = 0; i < 12; i++) sum += Number(base12[i]) * (i % 2 === 0 ? 1 : 3);
  return base12 + String((10 - (sum % 10)) % 10);
}

const FLOW = "bulk: 30 goods (manual + barcode) → vat → 30-item bill → qr";

test(`${FLOW} — desktop`, async ({ page }) => {
  await runFlow(page);
});

test(`${FLOW} — mobile`, async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await runFlow(page, "bulk_flow_mobile");
});

async function runFlow(page: any, _dir = "bulk_flow") {
  test.setTimeout(240_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  // 01: wallet sign-in via UI + subscribe
  await shot(page, "bulk_flow", "01-register-login-via-ui");
  await gotoUi(page);
  await uiSignIn(page);
  await page.locator('[aria-label="new-shop"]').click();
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required", { timeout: 15_000 });
  await uiSubscribe(page);
  await expect(page.locator('input[placeholder="new shop name"]')).toBeVisible({ timeout: 20_000 });

  // 02: create shop via modal
  await shot(page, "bulk_flow", "02-create-shop-via-modal");
  await page.locator('input[placeholder="new shop name"]').fill(`Bulk Cafe ${rand()}`);
  await page.locator('[aria-label="create-shop"]').click();
  const shopChip = page.locator(".shop-chip").first();
  await expect(shopChip).toBeVisible({ timeout: 20_000 });
  await shopChip.click();
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });

  // 03: token via API for bulk setup
  await shot(page, "bulk_flow", "03-token-via-api-for-bulk-setup");
  const { token } = await apiAuth(api, wallet);
  const shops = await (await api("GET", "/shops", token)).json();
  const shopId = shops[0].shop_id;

  // 04: destination via API (checkout needs a receive account)
  await shot(page, "bulk_flow", "04-destination-via-api-checkout-needs-a-rec");
  const dest = await api("POST", `/shops/${shopId}/destinations`, token, {
    shop_id: shopId,
    chain: "solana",
    address: "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR",
  });
  expect(dest.status()).toBe(200);

  // 05: add 100 kinds of goods through the UI form — 0..049 manual-only, 050..099 with barcode
  await shot(page, "bulk_flow", "05-add-100-kinds-of-goods-through-the-ui-fo");
  for (let i = 0; i < GOODS_UI_COUNT; i++) {
    const name = `item-${String(i).padStart(3, "0")}`;
    await page.locator('[aria-label="tab-goods"]').click();
    await page.locator('[aria-label="create-goods"]').click();
    await page.locator('input[placeholder="name"]').fill(name);
    await page.locator('input[placeholder="price"]').fill(PRICE);
    await page.locator('input[placeholder="cost"]').fill("1");
    await page.locator('input[placeholder="units"]').fill(String(STOCK_UNITS));
    if (i >= SCAN_START) {
      await page
        .locator('input[placeholder="barcode (optional)"]')
        .fill(ean13(`24${String(i).padStart(10, "0")}`));
    }
    await page.locator('[aria-label="add-goods"]').click();
    await expect(page.locator(".modal")).toHaveCount(0, { timeout: 20_000 });
  }
  const listed = await (await api("GET", `/shops/${shopId}/goods`, token)).json();
  expect(listed.length).toBe(GOODS_UI_COUNT);
  console.log(`[bulk] ${listed.length} goods added via UI form`);

  // capture the 30-kind table: page forward to the last page (item-029), pager state
  for (let p = 0; p < 5 && (await page.locator("tr", { hasText: "item-029" }).count()) === 0; p++) {
    await page.locator(".pager-btn").last().click();
  }
  await expect(page.locator("tr", { hasText: "item-029" })).toBeVisible({ timeout: 15_000 });

  // 07: VAT setting via UI (exclusive 10%)
  await page.locator('[aria-label="tab-settings"]').click();
  await page.locator('[aria-label="section-tax"]').click();
  await page.locator('[aria-label="vat-mode"]').selectOption("Exclusive");
  await page.locator('[aria-label="vat-rate"]').fill(VAT_RATE);
  await shot(page, "bulk_flow", "07-vat-setting-via-ui-exclusive-10");
  await page.locator('[aria-label="save-vat"]').click();
  await expect(page.locator(".toast.success", { hasText: "Payment settings saved" })).toBeVisible({
    timeout: 15_000,
  });

  // 08: sell tab → manual select 15 tiles
  await shot(page, "bulk_flow", "08-sell-tab-manual-select-15-tiles");
  await page.locator('[aria-label="tab-sell"]').click();
  await expect(page.locator(".goods-tile").first()).toBeVisible({ timeout: 20_000 });
  const tiles = await page.locator(".goods-tile").count();
  expect(tiles).toBe(GOODS_UI_COUNT);
  for (let i = 0; i < MANUAL_CLICK_COUNT; i++) {
    await page.locator(".goods-tile", { hasText: `item-${String(i).padStart(3, "0")}` }).click();
  }
  await expect(page.locator(".total-bar")).toContainText(String(MANUAL_CLICK_COUNT * Number(PRICE)));
  await expect(page.locator(".total-bar")).toContainText(`VAT ${VAT_RATE}%`);
  await shot(page, "bulk_flow", "08b-total-bar-shows-vat-breakdown");

  // 09: barcode scan 15 (scan input is always visible on the sell tab)
  await shot(page, "bulk_flow", "09-barcode-scan-15-scan-input-is-always-vis");
  const scan = page.locator('input[placeholder="scan or type barcode, Enter to add"]');
  for (let i = SCAN_START; i < SCAN_START + SCAN_COUNT; i++) {
    await scan.fill(ean13(`24${String(i).padStart(10, "0")}`));
    await scan.press("Enter");
  }
  const pickedCount = MANUAL_CLICK_COUNT + SCAN_COUNT;
  const subtotal = pickedCount * Number(PRICE);
  await expect(page.locator(".total-bar")).toContainText(String(subtotal));
  await shot(page, "bulk_flow", "09b-all-30-picked-after-scan");

  // 10: checkout → QR with VAT
  await shot(page, "bulk_flow", "10-checkout-qr-with-vat");
  await page.locator('[aria-label="new-doc"]').click();
  await expect(page.locator('[aria-label="pay-method"]').first()).toBeVisible({ timeout: 20_000 });
  await page.locator('[aria-label="pay-method"]').first().click();
  const qr = page.locator(".qr svg").first();
  await expect(qr).toBeVisible({ timeout: 20_000 });
  const expectedTotal = subtotal + vat_amount(subtotal);
  await expect(page.locator(".pay-total").first()).toContainText(String(expectedTotal));
  await expect(page.locator(".pay").first()).toContainText(`VAT ${VAT_RATE}%`);

  // 11: finish QR → back to a fresh sale (tab remount resets the charge view)
  await shot(page, "bulk_flow", "11-finish-qr-back-to-a-fresh-sale-tab-remou");
  await page.locator('[aria-label="close-modal"]').click();
  await page.locator('[aria-label="tab-goods"]').click();
  await page.locator('[aria-label="tab-sell"]').click();
  await expect(page.locator(".goods-grid")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".qr")).toHaveCount(0);

  // 12: billing persisted with VAT fields — one line per kind sold
  await shot(page, "bulk_flow", "12-billing-persisted-with-vat-fields-one-li");
  const billings = await (await api("GET", `/shops/${shopId}/billings`, token)).json();
  expect(billings).toHaveLength(1);
  expect(billings[0].vat_mode).toBe("exclusive");
  expect(Number(billings[0].vat_rate)).toBe(Number(VAT_RATE));
  expect(Number(billings[0].total)).toBeCloseTo(expectedTotal, 2);
  expect(Number(billings[0].vat_amount)).toBeCloseTo(vat_amount(subtotal), 2);
  expect(billings[0].lines).toHaveLength(pickedCount);

  // 13: every kind sold exactly once — stock decremented 1000 → 999
  await shot(page, "bulk_flow", "13-every-kind-sold-exactly-once-stock-decre");
  const after = await (await api("GET", `/shops/${shopId}/goods`, token)).json();
  expect(after).toHaveLength(GOODS_UI_COUNT);
  for (const g of after) {
    expect(g.units).toBe(SOLD_UNITS);
  }

  // 14: capture the billing PDF served by the backend
  const pdf = await api("GET", billings[0].pdf_url, token);
  expect(pdf.status()).toBe(200);
  const pdfBuf = Buffer.from(await pdf.body());
  expect(pdfBuf.subarray(0, 5).toString()).toBe("%PDF-");
  console.log(`[pdf] ${pdfBuf.length} bytes`);
}
