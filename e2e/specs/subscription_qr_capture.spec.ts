import path from "path";
import { test } from "@playwright/test";
import { freshWallet } from "./helpers/solana";
import { gotoUi, injectMockWallet, uiSignIn } from "./helpers/ui";

const out = path.join(__dirname, "..", "captures", "subscription_qr");
let shotNo = 0;

async function shot(page: any, name: string) {
  shotNo += 1;
  await page.screenshot({
    path: path.join(out, `${String(shotNo).padStart(2, "0")}-${name}.png`),
    fullPage: true,
  });
}

test("capture subscription pending panel with QR", async ({ page }) => {
  test.setTimeout(90_000);
  await injectMockWallet(page, freshWallet());
  await gotoUi(page);
  await uiSignIn(page);
  await page.locator('[aria-label="new-shop"]').click();
  await page.locator('input[placeholder="new shop name"]').fill("QR Capture Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  await page.locator(".modal-overlay").getByText("Subscription required").waitFor({ timeout: 20_000 });
  await page.locator('[aria-label="subscribe"]').click();
  await page
    .locator('input[placeholder="paste transaction signature after paying"]')
    .waitFor({ timeout: 15_000 });
  await page.locator(".qr").first().waitFor({ timeout: 10_000 });
  await shot(page, "subscription-pending-qr");
});
