import { test, expect } from "@playwright/test";
import { conn, ensureUsdc, freshWallet } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe } from "./helpers/ui";

// Flow rules: the subscription gate blocks BEFORE the shop — an unpaid user
// cannot enter; paying through the gate enters the pending shop.

test("gate inside shop when unpaid; no gate after subscribing", async ({ page }) => {
  test.setTimeout(120_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  // Unpaid: the create form opens directly (no gate at creation)…
  await gotoUi(page);
  await uiSignIn(page);
  await page.locator('[aria-label="new-shop"]').click();
  await expect(page.locator('input[placeholder="new shop name"]').first()).toBeVisible({
    timeout: 20_000,
  });
  await expect(page.locator(".modal-overlay")).not.toContainText("Subscription required");

  // …but after creating, the subscription gate appears before entering.
  await page.locator('input[placeholder="new shop name"]').fill("Gate Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required", {
    timeout: 15_000,
  });

  // Subscribe at the gate → the pending shop opens.
  await uiSubscribe(page);
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".modal-overlay")).toHaveCount(0, { timeout: 20_000 });
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });

  // Reload: quota still applies for the active user (1 shop) — disabled.
  await page.reload();
  await uiSignIn(page);
  await expect(page.locator('[aria-label="new-shop"]')).toBeDisabled();

  // And the active user is not gated again on re-entry.
  const chip2 = page.locator(".shop-chip", { hasText: "Gate Cafe" });
  await expect(chip2).toBeVisible({ timeout: 20_000 });
  await chip2.click();
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".modal-overlay")).toHaveCount(0, { timeout: 10_000 });
});
