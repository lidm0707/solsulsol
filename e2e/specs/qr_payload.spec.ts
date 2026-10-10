import { test, expect } from "@playwright/test";
import { apiAuth, conn, ensureUsdc, freshWallet, paySubscription } from "./helpers/solana";

// Validates the checkout QR payload against the Solana Pay transfer-URI spec
// (https://github.com/solana-foundation/solana-pay/blob/master/SPEC.md) —
// a malformed payload is what makes Phantom show an error on scan.

const BACKEND = process.env.POS_BACKEND_URL ?? "http://localhost:18080";

const VALID_ADDRESS = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";
const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

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

/** Base58-decode and check decoded length (ed25519 pubkey = 32 bytes). */
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

let request: any;
test.beforeEach(({ request: r }) => {
  request = r;
});

test("checkout QR is a spec-compliant Solana Pay transfer URI", async () => {
  const wallet = freshWallet();
  await ensureUsdc(conn());
  const { token } = await apiAuth(api, wallet);
  const order = await api("POST", "/payments/orders", token, {});
  const { order_id } = await order.json();
  const sig = await paySubscription(conn());
  const verify = await api("POST", "/payments/verify", token, { order_id, signature: sig });
  expect(verify.status()).toBe(200);
  const shop = await api("POST", "/shops", token, { name: "QR Cafe" });
  const { shop_id }: { shop_id: string } = await shop.json();

  const goods = await api("POST", `/shops/${shop_id}/goods`, token, {
    shop_id,
    name: "latte",
    price: "3.50",
  });
  const { goods_id } = await goods.json();
  const dest = await api("POST", `/shops/${shop_id}/destinations`, token, {
    shop_id,
    chain: "solana",
    address: VALID_ADDRESS,
  });
  const { destination_id } = await dest.json();

  const res = await api("POST", `/shops/${shop_id}/checkout`, token, {
    shop_id,
    goods_ids: [goods_id],
    destination_id,
  });
  expect(res.status()).toBe(200);
  const invoice = await res.json();

  // scheme + recipient
  expect(invoice.qr_payload).toMatch(/^solana:[1-9A-HJ-NP-Za-km-z]+/);
  const url = new URL(invoice.qr_payload.replace(/^solana:/, "http://placeholder/"));
  expect(isSolanaPubkey(decodeURIComponent(url.pathname.slice(1)))).toBe(true);

  // amount: positive decimal, ≤9 decimals, equals invoice total
  const amount = url.searchParams.get("amount");
  expect(amount).not.toBeNull();
  expect(amount).toMatch(/^\d+(\.\d{1,9})?$/);
  expect(parseFloat(amount!)).toBeCloseTo(parseFloat(invoice.total), 6);

  expect(url.searchParams.get("message")).toBeTruthy();

  // non-Solana destinations keep the legacy pay: scheme
  const btc = await api("POST", `/shops/${shop_id}/destinations`, token, {
    shop_id,
    chain: "btc",
    address: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
  });
  const { destination_id: btc_id } = await btc.json();
  const res2 = await api("POST", `/shops/${shop_id}/checkout`, token, {
    shop_id,
    goods_ids: [goods_id],
    destination_id: btc_id,
  });
  const inv2 = await res2.json();
  expect(inv2.qr_payload).toContain("pay:btc:");
});
