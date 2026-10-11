// Demo recording: product-first flow showing ALL features — the wallet
// subscribes BEFORE the recording starts, so no paywall shows. Beats:
// shop → goods (icon/color/barcode) → receive accounts (Solana + PromptPay)
// → VAT + THB fx → sell (scan, sort, hold bill) → 2 pay methods + QRs →
// reports/documents → subscription closing shot.
//   HUMAN_SPEED / HUMAN_TYPE_SPEED (ms) tune the pacing.
//   Run: cd e2e && npx playwright test demo_human_flow
import { test, expect } from "@playwright/test";
import { conn, ensureUsdc, freshWallet, paySubscription } from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, uiSubscribe, saveVideo } from "./helpers/ui";
import { pause, step, humanType, humanClick, expectVisible } from "./helpers/human";

const ADDRESS = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";
const PROMPTPAY_ADDRESS = "1111111111";
const ESPRESSO_BARCODE = "4006381333931";
const FX_THB_PER_USD = "3500";
const VAT_RATE = "7";
const VIDEO_TIMEOUT_MS = 420_000;

test.afterEach(async ({ page }) => {
  await saveVideo(page, "demo_human_flow");
});

const FLOW =
  "demo: signin → shop → goods(+icon/color/barcode) → receive(sol+promptpay) → vat/fx → sell(scan+sort+hold) → qr(sol+promptpay) → reports → subscription shot";

test(`${FLOW} — desktop`, async ({ page }) => {
  test.setTimeout(VIDEO_TIMEOUT_MS);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);

  // off-camera: subscribe BEFORE the recording matters, so no paywall shows
  await gotoUi(page);
  await uiSignIn(page);
  await humanClick(page, page.locator('[aria-label="new-shop"]'));
  await humanType(page, page.locator('input[placeholder="new shop name"]'), "Full Flow Cafe");
  await humanClick(page, page.locator('[aria-label="create-shop"]'));
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required", {
    timeout: 15_000,
  });
  await uiSubscribe(page);
  await expect(page.locator(".modal-overlay")).toHaveCount(0, { timeout: 20_000 });
  await step(page, "wallet pre-subscribed — recording the product flow");

  // 01 — inside the shop (creation auto-enters; gate cleared above)
  await expectVisible(page.locator(".tabbar"), 20_000);
  await step(page, "inside the shop");

  // 02 — goods: espresso with icon, color and a barcode; latte as a drink
  await humanClick(page, page.locator('[aria-label="tab-goods"]'));
  await step(page, "goods tab");
  await humanClick(page, page.locator('[aria-label="create-goods"]'));
  await step(page, "new goods form open");
  await humanType(page, page.locator('input[placeholder="name"]'), "espresso");
  await humanType(page, page.locator('input[placeholder="price"]'), "3.5");
  await humanType(page, page.locator('input[placeholder="cost"]'), "1.2");
  await humanType(page, page.locator('input[placeholder="units"]'), "10");
  await page.locator('[aria-label="goods-type"]').selectOption("food");
  await humanType(
    page,
    page.locator('input[placeholder="barcode (optional)"]'),
    ESPRESSO_BARCODE,
  );
  await page.locator('[aria-label="goods-icon"]').selectOption("coffee");
  await page.locator('[aria-label="goods-color"]').selectOption("c03");
  await pause(page);
  await step(page, "espresso filled — icon, color, barcode set, saving");
  await humanClick(page, page.locator('[aria-label="add-goods"]'));
  await expectVisible(page.locator("tr", { hasText: "espresso" }), 15_000);
  await step(page, "espresso saved");

  await humanClick(page, page.locator('[aria-label="create-goods"]'));
  await humanType(page, page.locator('input[placeholder="name"]'), "latte");
  await humanType(page, page.locator('input[placeholder="price"]'), "4");
  await humanType(page, page.locator('input[placeholder="cost"]'), "1.5");
  await humanType(page, page.locator('input[placeholder="units"]'), "8");
  await page.locator('[aria-label="goods-type"]').selectOption("drink");
  await page.locator('[aria-label="goods-icon"]').selectOption("soda");
  await page.locator('[aria-label="goods-color"]').selectOption("c05");
  await pause(page);
  await step(page, "latte filled, saving");
  await humanClick(page, page.locator('[aria-label="add-goods"]'));
  await expectVisible(page.locator("tr", { hasText: "latte" }), 15_000);
  await step(page, "latte saved");

  // 03 — receive accounts: Solana wallet + PromptPay (Thai bank)
  await humanClick(page, page.locator('[aria-label="tab-settings"]'));
  await step(page, "settings tab");
  await humanClick(page, page.locator('[aria-label="section-add-account"]'));
  await step(page, "add account form open");
  await humanType(page, page.locator('input[placeholder="address"]'), ADDRESS);
  await step(page, "Solana address pasted, saving");
  await humanClick(page, page.locator('[aria-label="add-destination"]'));
  await expectVisible(page.locator(".list-row", { hasText: "Solana" }), 15_000);
  await step(page, "Solana receive account added");

  await humanClick(page, page.locator('[aria-label="section-add-account"]'));
  await page.locator('[aria-label="dest-chain"]').selectOption("Bank");
  await humanType(page, page.locator('input[placeholder="address"]'), PROMPTPAY_ADDRESS);
  await step(page, "PromptPay number pasted, saving");
  await humanClick(page, page.locator('[aria-label="add-destination"]'));
  await expectVisible(page.locator(".list-row", { hasText: "PromptPay" }), 15_000);
  await step(page, "PromptPay receive account added");

  // 04 — tax: VAT inclusive 7% + the THB-per-$1 swap rate
  await humanClick(page, page.locator('[aria-label="section-tax"]'));
  await step(page, "tax settings");
  await page.locator('[aria-label="vat-mode"]').selectOption("Inclusive");
  await humanType(page, page.locator('[aria-label="vat-rate"]'), VAT_RATE);
  await humanType(page, page.locator('[aria-label="fx-thb-per-usd"]'), FX_THB_PER_USD);
  await pause(page);
  await step(page, "VAT 7% inclusive + 3500 THB per $1 swap rate, saving");
  await humanClick(page, page.locator('[aria-label="save-vat"]'));
  await expectVisible(
    page.locator(".toast.success", { hasText: "saved" }).first(),
    15_000,
  );
  await step(page, "tax settings saved");

  // 05 — sell: scan espresso by barcode, pick latte, sort by name
  await humanClick(page, page.locator('[aria-label="tab-sell"]'));
  const scan = page.locator('input[placeholder="scan or type barcode, Enter to add"]');
  await expectVisible(scan, 15_000);
  await step(page, "sell tab, scanning espresso's barcode");
  await humanType(page, scan, ESPRESSO_BARCODE);
  await scan.press("Enter");
  await expect(page.locator(".total-bar")).toContainText("3.5");
  await step(page, "espresso added by scan — picking latte too");
  await humanClick(page, page.locator(".goods-tile", { hasText: "latte" }));
  await expect(page.locator(".total-bar")).toContainText("7.5");
  await step(page, "two items in the cart — sorting goods by name");
  await page.locator('[aria-label="goods-sort"]').selectOption("name");
  await pause(page);
  await step(page, "goods sorted by name");

  // 06 — hold the bill, sell something else, recall the held bill
  await humanClick(page, page.locator('[aria-label="hold-bill"]'));
  await expectVisible(page.locator('[aria-label="held-bill-0"]'), 15_000);
  await step(page, "bill held — cart cleared, holding a second bill");
  await humanClick(page, page.locator(".goods-tile", { hasText: "espresso" }));
  await humanClick(page, page.locator('[aria-label="hold-bill"]'));
  await expectVisible(page.locator('[aria-label="held-bill-1"]'), 15_000);
  await step(page, "second bill held — recalling the first one");
  await humanClick(page, page.locator('[aria-label="held-bill-0"]'));
  await expect(page.locator(".total-bar")).toContainText("7.5");
  await step(page, "held bill back in the cart, confirming the bill");
  await humanClick(page, page.locator('[aria-label="new-doc"]'));
  await expectVisible(page.locator('[aria-label="pay-method"]').first(), 15_000);
  await pause(page);
  await step(page, "payment method sheet open — two receive accounts");

  // 07 — PromptPay checkout: THB total + VAT lines on the QR, then Reports
  await humanClick(
    page,
    page.locator('[aria-label="pay-method"]', { hasText: "PromptPay" }),
  );
  await expectVisible(page.locator(".qr svg").first(), 15_000);
  await expect(page.locator(".pay-total").first()).toContainText("฿");
  await expect(page.locator(".countdown").first()).toHaveText(/[45]:\d{2}/);
  await pause(page, 3_000);
  await step(page, "PromptPay QR shown in THB with VAT lines");
  await humanClick(page, page.locator('[aria-label="close-modal"]'));
  await expect(page.locator(".modal-overlay")).toHaveCount(0, { timeout: 15_000 });

  // 08 — reports: revenue/profit stats + the sales document
  await humanClick(page, page.locator('[aria-label="tab-reports"]'));
  await expectVisible(page.locator(".stat-value").first(), 15_000);
  await expect(page.locator(".list-row", { hasText: "espresso" })).toBeVisible({
    timeout: 15_000,
  });
  await pause(page, 2_000);
  await step(page, "reports — revenue, profit and the sales document");

  // 09 — closing shot: the $1/mo subscription lives in Settings
  await humanClick(page, page.locator('[aria-label="tab-settings"]'));
  await humanClick(page, page.locator('[aria-label="group-subscription"]'));
  await expectVisible(
    page.locator(".settings-body .card", { hasText: "$" }).first(),
    15_000,
  );
  await pause(page, 3_000);
  await step(page, "subscription — $1/month keeps the shop running. done");
});
