import { test, expect } from "@playwright/test";
import { apiAuth, conn, ensureUsdc, freshWallet } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe, saveVideo } from "./helpers/ui";

test.afterEach(async ({ page }) => {
  await saveVideo(page, "currency_flow");
});

const BACKEND = process.env.BACKEND ?? "http://localhost:18080";
const ADDRESS = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";
const PRICE = "4";
const FX = "36";

// currency: switch shop between THB and USDT and watch the sale total follow.
// video is recorded for every spec (see playwright.config.ts `video: on`);
// this one is kept in test-results as currency-setting demo.
const FLOW = "currency: THB ⇄ USDT — settings select, total bar symbol, api persistence";

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
  await expect(page.locator('input[placeholder="new shop name"]')).toBeVisible({ timeout: 20_000 });
  // Subscription gate opens after entering the shop; subscribe there.
  await page.locator('input[placeholder="new shop name"]').fill("Currency Mart");
  await page.locator('[aria-label="create-shop"]').click();
  // Unpaid: no entry — the gate blocks before the shop; pay to enter.
  await uiSubscribe(page);
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 15_000 });

  const { token } = await apiAuth(api, wallet);
  const shops = await (await api("GET", "/shops", token)).json();
  const shopId = shops[0].shop_id;
  await api("POST", `/shops/${shopId}/destinations`, token, {
    shop_id: shopId,
    chain: "solana",
    address: ADDRESS,
  });

  // 02: one good + swap rate so the converted QR amounts have a value
  await page.locator('[aria-label="tab-goods"]').click();
  await page.locator('[aria-label="create-goods"]').click();
  await page.locator('input[placeholder="name"]').fill("tea");
  await page.locator('input[placeholder="price"]').fill(PRICE);
  await page.locator('input[placeholder="cost"]').fill("1");
  await page.locator('input[placeholder="units"]').fill("10");
  await page.locator('[aria-label="add-goods"]').click();
  await expect(page.locator("tr", { hasText: "tea" })).toBeVisible({ timeout: 15_000 });

  const setCurrency = async (value: "Thb" | "Usdt", fx?: string) => {
    await page.locator('[aria-label="tab-settings"]').click();
    await page.locator('[aria-label="section-tax"]').click();
    if (fx !== undefined) {
      await page.locator('[aria-label="fx-thb-per-usd"]').fill(fx);
    }
    await page.locator('[aria-label="currency"]').selectOption(value);
    await page.locator('[aria-label="save-vat"]').click();
    await expect(page.locator(".toast.success", { hasText: "Payment settings saved" })).toBeVisible({
      timeout: 15_000,
    });
  };

  // 03: THB — total bar shows the baht amount (prices are already THB)
  await setCurrency("Thb", FX);
  await page.locator('[aria-label="tab-sell"]').click();
  const tile = page.locator(".goods-tile", { hasText: "tea" });
  await expect(tile).toBeVisible({ timeout: 15_000 });
  await tile.click();
  await expect(page.locator(".total-bar .pay-total")).toContainText("฿");
  await expect(page.locator(".total-bar .pay-total")).toContainText(PRICE);
  let shop = (await (await api("GET", "/shops", token)).json())[0];
  expect(shop.currency).toBe("thb");

  // 04: USDT — total bar shows the dollar amount
  await setCurrency("Usdt");
  await page.locator('[aria-label="tab-sell"]').click();
  const tile2 = page.locator(".goods-tile", { hasText: "tea" });
  await expect(tile2).toBeVisible({ timeout: 15_000 });
  await tile2.click();
  await expect(page.locator(".total-bar .pay-total")).toContainText("$");
  await expect(page.locator(".total-bar .pay-total")).toContainText(PRICE);
  shop = (await (await api("GET", "/shops", token)).json())[0];
  expect(shop.currency).toBe("usdt");
});
