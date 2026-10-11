import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { conn, ensureUsdc, freshWallet } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe, saveVideo } from "./helpers/ui";

test.afterEach(async ({ page }) => {
  await saveVideo(page, "scan_goods");
});

let stepNo = 0;
async function shot(page: any, dir: string, name: string) {
  stepNo += 1;
  const out = path.join(__dirname, "..", "captures", dir);
  fs.mkdirSync(out, { recursive: true });
  await page.screenshot({ path: path.join(out, stepNo + "-" + name + ".png"), fullPage: true });
}

const EAN13 = "4006381333931";
const UNKNOWN_CODE = "73513537";

const FLOW = "ui: scan goods by barcode on sell tab";

test(`${FLOW}`, async ({ page }) => {
  test.setTimeout(90_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  // 01: wallet sign-in + subscribe
  await gotoUi(page);
  await uiSignIn(page);
  await page.locator('[aria-label="new-shop"]').click();
  await expect(page.locator('input[placeholder="new shop name"]')).toBeVisible({ timeout: 20_000 });
  // Subscription gate opens after entering the shop; subscribe there.

  // 02: create shop
  await page.locator('input[placeholder="new shop name"]').fill("Scan Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  // Unpaid: no entry — the gate blocks before the shop; pay to enter.
  await uiSubscribe(page);
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 15_000 });

  // 03: goods tab → create goods with barcode
  await page.locator('[aria-label="tab-goods"]').click();
  await page.locator('[aria-label="create-goods"]').click();
  await page.locator('input[placeholder="name"]').fill("espresso");
  await page.locator('input[placeholder="price"]').fill("3.5");
  await page.locator('input[placeholder="cost"]').fill("1.2");
  await page.locator('input[placeholder="units"]').fill("10");
  await page.locator('input[placeholder="barcode (optional)"]').fill(EAN13);
  await page.locator('[aria-label="add-goods"]').click();
  await expect(page.locator("tr", { hasText: "espresso" })).toBeVisible({ timeout: 15_000 });
  await expect(page.locator("tr", { hasText: "espresso" })).toContainText(EAN13);

  // 04: sell tab → scan barcode to pick the goods
  await shot(page, "scan_goods", "04-sell-tab-before-scan");
  await page.locator('[aria-label="tab-sell"]').click();
  const scanInput = page.locator('input[placeholder="scan or type barcode, Enter to add"]');
  await expect(scanInput).toBeVisible({ timeout: 15_000 });
  await scanInput.click();
  await scanInput.pressSequentially(EAN13);
  await scanInput.press("Enter");
  const tile = page.locator(".goods-tile", { hasText: "espresso" });
  await expect(tile).toHaveClass(/picked/, { timeout: 15_000 });
  await expect(page.locator(".total-bar")).toContainText("3.5");
  await shot(page, "scan_goods", "05-scan-picked-tile");

  // 05: unknown barcode → error toast, nothing picked twice
  await scanInput.click();
  await scanInput.pressSequentially(UNKNOWN_CODE);
  await scanInput.press("Enter");
  await expect(page.locator(".toast.error")).toContainText(`unknown barcode: ${UNKNOWN_CODE}`);
  await shot(page, "scan_goods", "06-unknown-barcode-toast");
});
