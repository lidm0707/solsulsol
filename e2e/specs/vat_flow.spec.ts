import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { apiAuth, conn, ensureUsdc, freshWallet } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe, saveVideo } from "./helpers/ui";

test.afterEach(async ({ page }) => {
  await saveVideo(page, "vat_flow");
});

let stepNo = 0;
async function shot(page: any, dir: string, name: string) {
  stepNo += 1;
  const out = path.join(__dirname, "..", "captures", dir);
  fs.mkdirSync(out, { recursive: true });
  await page.screenshot({ path: path.join(out, stepNo + "-" + name + ".png"), fullPage: true });
}

const BACKEND = process.env.BACKEND ?? "http://localhost:18080";
const ADDRESS = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";
const VAT_RATE = "10";
const EAN13 = "4006381333931";

const EXCLUSIVE_TOTAL = "3.85"; // 3.5 + 10%
const EXCLUSIVE_VAT = "0.35";
const INCLUSIVE_VAT = "0.36"; // 4 * 10 / 110, rounded to 2dp

const FLOW = "vat: exclusive + inclusive — form, total bar, checkout qr, persisted billing";

test(`${FLOW}`, async ({ page }) => {
  test.setTimeout(120_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  let request: any = page.request;
  const api = async (method: string, apiPath: string, token?: string, body?: unknown) =>
    request.fetch(`${BACKEND}${apiPath}`, {
      method,
      headers: {
        "Content-Type": "application/json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      data: body ? JSON.stringify(body) : undefined,
    });

  // 01: wallet sign-in + subscribe + shop
  await gotoUi(page);
  await uiSignIn(page);
  await page.locator('[aria-label="new-shop"]').click();
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required", { timeout: 15_000 });
  await uiSubscribe(page);
  await expect(page.locator('input[placeholder="new shop name"]')).toBeVisible({ timeout: 20_000 });
  await page.locator('input[placeholder="new shop name"]').fill("VAT Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  const shopChip = page.locator(".shop-chip", { hasText: "VAT Cafe" });
  await expect(shopChip).toBeVisible({ timeout: 15_000 });
  await shopChip.click();
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 15_000 });

  const { token } = await apiAuth(api, wallet);
  const shops = await (await api("GET", "/shops", token)).json();
  const shopId = shops[0].shop_id;
  const dest = await api("POST", `/shops/${shopId}/destinations`, token, {
    shop_id: shopId,
    chain: "solana",
    address: ADDRESS,
  });
  expect(dest.status()).toBe(200);

  // 02: two goods — espresso carries a barcode so the scan-to-add is captured
  for (const [name, price] of [["espresso", "3.5"], ["latte", "4"]] as const) {
    await page.locator('[aria-label="tab-goods"]').click();
    await page.locator('[aria-label="create-goods"]').click();
    await page.locator('input[placeholder="name"]').fill(name);
    await page.locator('input[placeholder="price"]').fill(price);
    await page.locator('input[placeholder="cost"]').fill("1");
    await page.locator('input[placeholder="units"]').fill("10");
    if (name === "espresso") {
      await page.locator('input[placeholder="barcode (optional)"]').fill(EAN13);
    }
    await page.locator('[aria-label="add-goods"]').click();
    await expect(page.locator("tr", { hasText: name })).toBeVisible({ timeout: 15_000 });
  }

  const setVat = async (mode: "Exclusive" | "Inclusive") => {
    await page.locator('[aria-label="tab-settings"]').click();
    await page.locator('[aria-label="section-tax"]').click();
    await page.locator('[aria-label="vat-mode"]').selectOption(mode);
    await page.locator('[aria-label="vat-rate"]').fill(VAT_RATE);
  };
  const checkoutQr = async () => {
    await page.locator('[aria-label="tab-sell"]').click();
    await expect(page.locator(".goods-tile").first()).toBeVisible({ timeout: 15_000 });
    await page.locator('[aria-label="new-doc"]').click();
    await expect(page.locator('[aria-label="pay-method"]').first()).toBeVisible({ timeout: 20_000 });
    await page.locator('[aria-label="pay-method"]').first().click();
    await expect(page.locator(".qr svg").first()).toBeVisible({ timeout: 20_000 });
  };
  const freshSale = async () => {
    await page.locator('[aria-label="close-modal"]').click();
    await page.locator('[aria-label="tab-goods"]').click();
    await page.locator('[aria-label="tab-sell"]').click();
    await expect(page.locator(".goods-grid")).toBeVisible({ timeout: 15_000 });
    await expect(page.locator(".qr")).toHaveCount(0);
  };

  // ---- exclusive ----
  await setVat("Exclusive");
  await shot(page, "vat_flow", "01-vat-exclusive-form-filled");
  await page.locator('[aria-label="save-vat"]').click();
  await expect(page.locator(".toast.success", { hasText: "Payment settings saved" })).toBeVisible({
    timeout: 15_000,
  });

  // 03: scan-to-add — the total bar shows the exclusive VAT breakdown
  await page.locator('[aria-label="tab-sell"]').click();
  const scan = page.locator('input[placeholder="scan or type barcode, Enter to add"]');
  await expect(scan).toBeVisible({ timeout: 15_000 });
  await scan.click();
  await scan.pressSequentially(EAN13);
  await scan.press("Enter");
  await expect(page.locator(".goods-tile", { hasText: "espresso" })).toHaveClass(/picked/, {
    timeout: 15_000,
  });
  const totalBar = page.locator(".total-bar");
  await expect(totalBar).toContainText("Subtotal");
  await expect(totalBar).toContainText(`VAT ${VAT_RATE}%`);
  await expect(totalBar).toContainText(EXCLUSIVE_VAT);
  await expect(totalBar).toContainText(EXCLUSIVE_TOTAL);
  await shot(page, "vat_flow", "02-vat-exclusive-total-bar-breakdown");

  // 04: checkout QR with the VAT line and the added-on-top total
  await checkoutQr();
  await expect(page.locator(".pay").first()).toContainText(`VAT ${VAT_RATE}%`);
  await expect(page.locator(".pay-total").first()).toContainText(EXCLUSIVE_TOTAL);
  await shot(page, "vat_flow", "03-vat-exclusive-checkout-qr");

  // 05: persisted billing — exclusive fields
  let billings = await (await api("GET", `/shops/${shopId}/billings`, token)).json();
  expect(billings).toHaveLength(1);
  expect(billings[0].vat_mode).toBe("exclusive");
  expect(Number(billings[0].vat_rate)).toBe(Number(VAT_RATE));
  expect(Number(billings[0].subtotal)).toBe(3.5);
  expect(Number(billings[0].vat_amount)).toBeCloseTo(Number(EXCLUSIVE_VAT), 2);
  expect(Number(billings[0].total)).toBeCloseTo(Number(EXCLUSIVE_TOTAL), 2);

  await freshSale();

  // ---- inclusive ----
  await setVat("Inclusive");
  await shot(page, "vat_flow", "04-vat-inclusive-form-filled");
  await page.locator('[aria-label="save-vat"]').click();
  await expect(page.locator(".toast.success", { hasText: "Payment settings saved" })).toBeVisible({
    timeout: 15_000,
  });

  // 06: pick latte — total stays 4, VAT shown as included portion
  await page.locator('[aria-label="tab-sell"]').click();
  const latte = page.locator(".goods-tile", { hasText: "latte" });
  await expect(latte).toBeVisible({ timeout: 15_000 });
  await latte.click();
  const totalBar2 = page.locator(".total-bar");
  await expect(totalBar2).toContainText("Subtotal");
  await expect(totalBar2).toContainText(`VAT ${VAT_RATE}%`);
  await expect(totalBar2).toContainText(INCLUSIVE_VAT);
  await expect(totalBar2).toContainText("4");
  await shot(page, "vat_flow", "05-vat-inclusive-total-bar-breakdown");

  // 07: checkout QR — total is the price itself, VAT included
  await checkoutQr();
  await expect(page.locator(".pay").first()).toContainText(`VAT ${VAT_RATE}%`);
  await expect(page.locator(".pay-total").first()).toContainText("4");
  await shot(page, "vat_flow", "06-vat-inclusive-checkout-qr");

  // 08: persisted billing — inclusive fields
  billings = await (await api("GET", `/shops/${shopId}/billings`, token)).json();
  expect(billings).toHaveLength(2);
  const inclusive = billings.find((b: any) => b.vat_mode === "inclusive");
  expect(inclusive).toBeTruthy();
  expect(Number(inclusive.vat_rate)).toBe(Number(VAT_RATE));
  expect(Number(inclusive.subtotal)).toBe(4);
  expect(Number(inclusive.vat_amount)).toBeCloseTo(Number(INCLUSIVE_VAT), 2);
  expect(Number(inclusive.total)).toBe(4);
});
