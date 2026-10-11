import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  apiAuth,
  conn,
  ensureUsdc,
  freshWallet,
  paySubscription,
} from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn } from "./helpers/ui";

// Receipt QR coverage for every destination type: payload assertions at the
// API level + a UI capture of each receipt QR modal.
//
//  solana USDC/USDT → solana:<addr>?amount=..&spl-token=<mint>
//  promptpay (THB) → EMVCo payload (00 02 01, AID A000000677010111, tag 54)
//  eth/btc/binance → pay:<chain>:<addr>:<destination_id>

const BACKEND = process.env.POS_BACKEND_URL ?? "http://localhost:18080";
const USDC_ADDR = "Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuGzaGVcPboggRNA";
const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const FX_THB_PER_USD = "3500";
const GOODS_NAME = "latte";
const GOODS_PRICE = "3.50";
// THB shop: PromptPay QR = the THB total as-is; Solana QR = total / fx in
// USD-token units.
const PROMPTPAY_THB = "3.50";

function api(method: string, path: string, token?: string, body?: unknown) {
  return request.fetch(`${BACKEND}${path}`, {
    method,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    data: body ? JSON.stringify(body) : undefined,
  });
}

function isSolanaPubkey(s: string): boolean {
  if (!s.length || [...s].some((c) => !B58.includes(c))) return false;
  let num = 0n;
  for (const c of s) num = num * 58n + BigInt(B58.indexOf(c));
  const bytes: number[] = [];
  while (num > 0n) {
    bytes.unshift(Number(num % 256n));
    num /= 256n;
  }
  for (let i = 0; i < s.length && s[i] === "1"; i++) bytes.unshift(0);
  return bytes.length === 32;
}

/** TLV parse of an EMVCo payload: value of a top-level tag. */
function emvcoTag(payload: string, tag: string): string | null {
  let i = 0;
  while (i + 4 <= payload.length) {
    const t = payload.slice(i, i + 2);
    const len = parseInt(payload.slice(i + 2, i + 4), 10);
    if (Number.isNaN(len)) return null;
    if (t === tag) return payload.slice(i + 4, i + 4 + len);
    i += 4 + len;
  }
  return null;
}

type QrType = {
  name: string;
  chain: string;
  token?: string;
  address: string;
};

const TYPES: QrType[] = [
  { name: "01-solana-usdc", chain: "solana", token: "usdc", address: USDC_ADDR },
  { name: "02-promptpay", chain: "bank", address: "1111111111" },
  {
    name: "03-eth",
    chain: "eth",
    address: "0x71C7656EC7ab88b098defB751B7401B5f6d8976F",
  },
  { name: "04-btc", chain: "btc", address: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4" },
  {
    name: "05-binance",
    chain: "binance",
    address: "bnb136nsxpqxaea9xmwlghm4wxxrvuhvvk8nr9u6j2",
  },
];

// The UI method tiles show only the first 4 chars of the address.
const tileOf = (t: QrType) => t.address.slice(0, 4);

let request: any;
test.beforeEach(({ request: r }) => {
  request = r;
});

test("receipt QR for every destination type: payload spec + captures", async ({ page }) => {
  test.setTimeout(180_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  const { token } = await apiAuth(api, wallet);
  const order = await api("POST", "/payments/orders", token, {});
  const { order_id } = await order.json();
  const sig = await paySubscription(conn());
  expect(
    (await api("POST", "/payments/verify", token, { order_id, signature: sig })).status(),
  ).toBe(200);

  const shop = await api("POST", "/shops", token, { name: "QR Types Cafe" });
  const { shop_id }: { shop_id: string } = await shop.json();
  // PromptPay amounts are THB; the swap rate converts THB prices to the
  // USD-token amount on Solana QRs.
  expect(
    (
      await api("PUT", `/shops/${shop_id}/settings`, token, {
        vat_mode: "none",
        vat_rate: "0",
        fx_thb_per_usd: FX_THB_PER_USD,
        currency: "thb",
      })
    ).status(),
  ).toBe(200);

  const goods = await api("POST", `/shops/${shop_id}/goods`, token, {
    shop_id,
    name: GOODS_NAME,
    price: GOODS_PRICE,
  });
  const { goods_id } = await goods.json();

  // API: create every destination, checkout, validate the payload per type.
  for (const t of TYPES) {
    const dest = await api("POST", `/shops/${shop_id}/destinations`, token, {
      shop_id,
      chain: t.chain,
      address: t.address,
      ...(t.token ? { token: t.token } : {}),
    });
    expect(dest.status(), t.name).toBe(200);
    const { destination_id } = await dest.json();

    const res = await api("POST", `/shops/${shop_id}/checkout`, token, {
      shop_id,
      goods_ids: [goods_id],
      destination_id,
    });
    expect(res.status(), t.name).toBe(200);
    const inv = await res.json();

    if (t.chain === "solana") {
      expect(inv.qr_payload).toMatch(/^solana:[1-9A-HJ-NP-Za-km-z]+/);
      const url = new URL(inv.qr_payload.replace(/^solana:/, "http://placeholder/"));
      expect(isSolanaPubkey(decodeURIComponent(url.pathname.slice(1)))).toBe(true);
      const amount = url.searchParams.get("amount");
      expect(amount).toMatch(/^\d+(\.\d{1,9})?$/);
      expect(parseFloat(amount!)).toBeCloseTo(Number(inv.qr_amount), 6);
      expect(url.searchParams.get("message")).toBeTruthy();
      if (t.token) {
        // Canonical mainnet USDC mint (SplToken::Usdc.mint()), not the
        // surfpool test mint.
        expect(url.searchParams.get("spl-token"), t.name)
          .toBe("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        expect(Number(inv.qr_amount)).toBeCloseTo(Number(inv.total) / 3500, 6);
      }
    } else if (t.chain === "bank") {
      expect(inv.qr_payload.startsWith("000201"), t.name).toBe(true);
      const aid = emvcoTag(inv.qr_payload, "29");
      expect(aid, "promptpay AID").toContain("A000000677010111");
      expect(emvcoTag(inv.qr_payload, "54"), "THB amount").toBe(PROMPTPAY_THB);
      expect(Number(inv.qr_amount)).toBe(3.5);
    } else {
      // Legacy payload uses the short domain chain label (eth/btc/bnb).
      const label = t.chain === "binance" ? "bnb" : t.chain;
      expect(inv.qr_payload, t.name).toBe(`pay:${label}:${t.address}:${destination_id}`);
      expect(Number(inv.qr_amount)).toBeCloseTo(Number(inv.total), 6);
    }
  }

  // UI: capture each receipt QR modal from the sell tab.
  await injectMockWallet(page, wallet);
  await gotoUi(page);
  await uiSignIn(page);
  const chip = page.locator(".shop-chip", { hasText: "QR Types Cafe" });
  await expect(chip).toBeVisible({ timeout: 20_000 });
  await chip.click();
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".goods-tile", { hasText: GOODS_NAME })).toBeVisible({
    timeout: 15_000,
  });

  let shotNo = 0;
  for (const t of TYPES) {
    // Toggling tabs resets the sell tab's charge state (QR shows inline).
    await page.locator('[aria-label="tab-goods"]').click();
    await page.locator('[aria-label="tab-sell"]').click();
    const tile = page.locator(".goods-tile", { hasText: GOODS_NAME });
    await tile.click();
    // Shop currency is THB, so the bar shows the baht total as-is.
    await expect(page.locator(".total-bar")).toContainText("3.5");
    await page.locator('[aria-label="new-doc"]').click();
    const method = page
      .locator('[aria-label="pay-method"]', { hasText: tileOf(t) })
      .first();
    await expect(method).toBeVisible({ timeout: 15_000 });
    await method.click();
    const qrEl = page.locator(".qr svg").first();
    await expect(qrEl).toBeVisible({ timeout: 15_000 });
    await page.waitForTimeout(300);
    shotNo += 1;
    const out = path.join(__dirname, "..", "captures", "qr_types");
    fs.mkdirSync(out, { recursive: true });
    await page.screenshot({
      path: path.join(out, `${String(shotNo).padStart(2, "0")}-${t.name.replace(/^\d+-/, "")}.png`),
      fullPage: true,
    });
    await page.locator('[aria-label="close-modal"]').click();
    await expect(page.locator(".modal-overlay")).toHaveCount(0, { timeout: 10_000 });
  }
});
