// Captures the user-manual screenshots (manual/images) against the real
// stack: one PNG per manual step. Run after `make e2e` stack is up:
//   npx playwright test specs/manual_captures.spec.ts
import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { conn, ensureUsdc, freshWallet } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe } from "./helpers/ui";

const OUT = path.join(__dirname, "..", "captures", "manual");
const ADDRESS = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";
const PROMPTPAY_ADDRESS = "1111111111";

async function shot(page: import("@playwright/test").Page, name: string) {
  fs.mkdirSync(OUT, { recursive: true });
  // dismiss any toasts so screenshots are clean
  for (let i = 0; i < 6; i++) {
    const close = page.locator('[aria-label="close-toast"]');
    if ((await close.count()) === 0) break;
    await close.first().click({ timeout: 500 }).catch(() => {});
    await page.waitForTimeout(200);
  }
  await page.waitForTimeout(800);
  await page.screenshot({ path: path.join(OUT, name), fullPage: true });
}

test("capture manual screenshots", async ({ page }) => {
  test.setTimeout(240_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  // 01 — wallet landing (sign in with your wallet)
  await gotoUi(page);
  await shot(page, "01-signin.png");
  await uiSignIn(page);

  // 04 — create your store
  await shot(page, "04-new-shop.png");
  await page.locator('[aria-label="new-shop"]').click();
  await page.locator('input[placeholder="new shop name"]').fill("Manual Cafe");
  await page.locator('[aria-label="create-shop"]').click();

  // 02 — subscription gate
  await expect(page.locator(".modal-overlay")).toContainText("Subscription", {
    timeout: 15_000,
  });
  await shot(page, "02-subscription.png");
  await uiSubscribe(page);

  // 03 — dashboard (new sale, empty)
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 15_000 });
  await shot(page, "03-dashboard.png");

  // 05 — goods: espresso + latte, then the goods table
  await page.locator('[aria-label="tab-goods"]').click();
  for (const [name, price, type, icon] of [
    ["espresso", "3.5", "food", "coffee"],
    ["latte", "4", "drink", "soda"],
  ] as const) {
    await page.locator('[aria-label="create-goods"]').click();
    await page.locator('input[placeholder="name"]').fill(name);
    await page.locator('input[placeholder="price"]').fill(price);
    await page.locator('input[placeholder="cost"]').fill("1.2");
    await page.locator('input[placeholder="units"]').fill("10");
    await page.locator('[aria-label="goods-type"]').selectOption(type);
    await page.locator('[aria-label="goods-icon"]').selectOption(icon);
    await page.locator('[aria-label="add-goods"]').click();
    await expect(page.locator("tr", { hasText: name })).toBeVisible({ timeout: 15_000 });
  }
  await shot(page, "05-goods.png");

  // 06 — receive accounts: Solana + PromptPay
  await page.locator('[aria-label="tab-settings"]').click();
  await page.locator('[aria-label="section-add-account"]').click();
  await page.locator('input[placeholder="address"]').fill(ADDRESS);
  await page.locator('[aria-label="add-destination"]').click();
  await expect(page.locator(".list-row", { hasText: "Solana" })).toBeVisible({ timeout: 15_000 });
  await page.locator('[aria-label="section-add-account"]').click();
  await page.locator('[aria-label="dest-chain"]').selectOption("Bank");
  await page.locator('input[placeholder="address"]').fill(PROMPTPAY_ADDRESS);
  await page.locator('[aria-label="add-destination"]').click();
  await expect(page.locator(".list-row", { hasText: "PromptPay" })).toBeVisible({
    timeout: 15_000,
  });
  await page.locator('[aria-label="section-receive"]').click();
  await shot(page, "06-receive-accounts.png");

  // swap rate so Solana QRs convert from THB prices
  await page.locator('[aria-label="section-tax"]').click();
  await page.locator('[aria-label="fx-thb-per-usd"]').fill("3500");
  await page.locator('[aria-label="save-vat"]').click();
  await expect(page.locator(".toast.success", { hasText: "saved" }).first()).toBeVisible({
    timeout: 15_000,
  });

  // 07 — a sale: pick espresso, open the payment method sheet
  await page.locator('[aria-label="tab-sell"]').click();
  await page.locator(".goods-tile", { hasText: "espresso" }).click();
  await expect(page.locator(".total-bar")).toContainText("3.5");
  await page.locator('[aria-label="new-doc"]').click();
  await expect(page.locator('[aria-label="pay-method"]').first()).toBeVisible({
    timeout: 15_000,
  });
  await shot(page, "07-choose-payment.png");

  // 08 — the billing QR (PromptPay, THB)
  await page
    .locator('[aria-label="pay-method"]', { hasText: "PromptPay" })
    .first()
    .click();
  await expect(page.locator(".qr svg").first()).toBeVisible({ timeout: 15_000 });
  await shot(page, "08-billing-qr.png");
  await page.locator('[aria-label="close-modal"]').click();
  await expect(page.locator(".modal-overlay")).toHaveCount(0, { timeout: 15_000 });

  // 09 — the sale is recorded: reports show revenue + the document
  await page.locator('[aria-label="tab-reports"]').click();
  await expect(page.locator(".stat-value").first()).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".list-row", { hasText: "espresso" })).toBeVisible({
    timeout: 15_000,
  });
  await shot(page, "09-paid.png");
});
