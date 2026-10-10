import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { conn, ensureUsdc, freshWallet } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe, saveVideo } from "./helpers/ui";

const ADDRESS = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";

let stepNo = 0;
async function shot(page: any, name: string) {
  stepNo += 1;
  const out = path.join(__dirname, "..", "captures", "full_workflow");
  fs.mkdirSync(out, { recursive: true });
  await page.screenshot({ path: path.join(out, stepNo + "-" + name + ".png"), fullPage: true });
}

test.afterEach(async ({ page }) => {
  await saveVideo(page, "full_workflow");
});

const FLOW =
  "full workflow: signin → create shop → sub gate in modal → pay → shop form → create → goods → receive → sell → qr";

test(`${FLOW} — desktop`, async ({ page }) => {
  test.setTimeout(120_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  // 01: sign in → dashboard (unpaid)
  await gotoUi(page);
  await uiSignIn(page);
  await shot(page, "01-signin-dashboard-unpaid");

  // 02: + New shop while unpaid → subscription gate modal, no name form
  await page.locator('[aria-label="new-shop"]').click();
  await expect(page.locator(".modal-overlay")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required");
  await expect(page.locator('input[placeholder="new shop name"]')).toHaveCount(0);
  await shot(page, "02-sub-gate-in-create-modal");

  // 03: pay $1 inside the modal → verify → shop form appears
  await uiSubscribe(page);
  await expect(page.locator('input[placeholder="new shop name"]')).toBeVisible({ timeout: 20_000 });
  await shot(page, "03-paid-shop-form-shown");

  // 04: create the shop
  await page.locator('input[placeholder="new shop name"]').fill("Full Flow Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  const shopChip = page.locator(".shop-chip", { hasText: "Full Flow Cafe" });
  await expect(shopChip).toBeVisible({ timeout: 15_000 });
  await shot(page, "04-shop-created");

  // 05: enter the shop
  await shopChip.click();
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".page-title")).toHaveText("New sale");
  await shot(page, "05-entered-shop");

  // 06: goods — add an item with price/cost/stock
  await page.locator('[aria-label="tab-goods"]').click();
  await page.locator('[aria-label="create-goods"]').click();
  await page.locator('input[placeholder="name"]').fill("espresso");
  await page.locator('input[placeholder="price"]').fill("3.5");
  await page.locator('input[placeholder="cost"]').fill("1.2");
  await page.locator('input[placeholder="units"]').fill("10");
  await page.locator('[aria-label="goods-type"]').selectOption("food");
  await page.locator('[aria-label="add-goods"]').click();
  await expect(page.locator("tr", { hasText: "espresso" })).toBeVisible({ timeout: 15_000 });
  await shot(page, "06-goods-created");

  // 07: receive account — add a Solana destination
  await page.locator('[aria-label="tab-settings"]').click();
  await page.locator('[aria-label="section-add-account"]').click();
  await page.locator('input[placeholder="address"]').fill(ADDRESS);
  await page.locator('[aria-label="add-destination"]').click();
  await expect(page.locator(".list-row", { hasText: "Solana" })).toBeVisible({ timeout: 15_000 });
  await shot(page, "07-receive-account-added");

  // 08: sell — pick the tile, confirm, method sheet, QR
  await page.locator('[aria-label="tab-sell"]').click();
  const tile = page.locator(".goods-tile", { hasText: "espresso" });
  await expect(tile).toBeVisible({ timeout: 15_000 });
  await tile.click();
  await expect(page.locator(".total-bar")).toContainText("3.5");
  await page.locator('[aria-label="new-doc"]').click();
  await expect(page.locator('[aria-label="pay-method"]').first()).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(300);
  await shot(page, "08-pay-method-sheet");
  await page.locator('[aria-label="pay-method"]').first().click();
  const qr = page.locator(".qr svg").first();
  await expect(qr).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".pay-total").first()).toHaveText(/3\.5/);
  await expect(page.locator(".countdown").first()).toHaveText(/[45]:\d{2}/);
  await shot(page, "09-billing-qr");
});
