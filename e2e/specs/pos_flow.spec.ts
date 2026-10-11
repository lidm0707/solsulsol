import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { Keypair } from "@solana/web3.js";
import { apiAuth, conn, ensureUsdc, freshWallet, paySubscription } from "./helpers/solana";

const BACKEND = process.env.BACKEND ?? "http://localhost:18080";
const SNAPDIR = path.join(__dirname, "..", "snapshots", "pos_flow");
const UPDATE = process.env.UPDATE_SNAPSHOTS === "1";

let request: any;
let stepNo = 0;
let failures: string[] = [];

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

/** Mask volatile values so snapshots stay deterministic across runs. */
function mask(value: unknown): unknown {
  const json = JSON.stringify(value, null, 2) ?? "";
  const masked = json
    .replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi, "<uuid>")
    .replace(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g, "<jwt>")
    .replace(/"signature":\s*"[^"]*"/g, '"signature": "<txsig>"')
    .replace(/"wallet":\s*"[^"]*"/g, '"wallet": "<wallet>"')
    .replace(/member-[0-9a-z]+@test\.dev/g, "member-<rand>@test.dev")
    // wallet-only members have no email; their label is the base58 pubkey
    .replace(/"email": "[1-9A-HJ-NP-Za-km-z]{32,48}"/g, '"email": "<wallet>"')
    .replace(
      /"(created_at|expires_at|started_at|grace_until)":\s*"[^"]*"/g,
      '"$1": "<ts>"',
    );
  return JSON.parse(masked);
}

function snapshot(name: string, payload: unknown) {
  stepNo += 1;
  const file = `step-${String(stepNo).padStart(2, "0")}-${name}.json`;
  const json = JSON.stringify(mask(payload), null, 2) + "\n";
  const target = path.join(SNAPDIR, file);
  fs.mkdirSync(SNAPDIR, { recursive: true });
  console.log(`[step ${stepNo}] ${name}`);
  if (UPDATE || !fs.existsSync(target)) {
    fs.writeFileSync(target, json);
    return;
  }
  const expected = fs.readFileSync(target, "utf8");
  if (expected !== json) {
    failures.push(file);
    console.log(`  MISMATCH vs ${file}\n--- expected ---\n${expected}\n--- actual ---\n${json}`);
  }
}

async function step(name: string, res: any) {
  const status = res.status();
  const body = await res.json().catch(() => null);
  snapshot(name, { status, body });
  expect({ step: name, status, failures: [...failures] }).toEqual({
    step: name,
    status,
    failures: [],
  });
  return body;
}

test.beforeEach(({ request: r }) => {
  request = r;
  stepNo = 0;
  failures = [];
});

test.afterEach(() => {
  expect(failures, "snapshot mismatches").toEqual([]);
});

test("wallet auth → subscribe → shop → invite → goods → checkout → PDF/QR", async () => {
  const owner = freshWallet();
  const member = Keypair.generate();
  const memberEmail = `member-${Math.random().toString(36).slice(2, 10)}@test.dev`;

  // 01–02: owner wallet auth (nonce → sign → verify)
  await ensureUsdc(conn());
  const auth = await apiAuth(api, owner);
  const token = auth.token;
  const me = await api("GET", "/auth/me", token);
  await step("auth-owner", me);

  // 03–05: $1/mo subscription — order, on-chain pay, verify
  const order = await api("POST", "/payments/orders", token, {});
  const { order_id } = await step("create-payment-order", order);
  const sig = await paySubscription(conn());
  const verify = await api("POST", "/payments/verify", token, {
    order_id,
    signature: sig,
  });
  const sub = await step("verify-payment", verify);
  expect(sub.status).toBe("active");

  // 06: create shop (requires active subscription)
  const shop = await api("POST", "/shops", token, { name: "E2E Cafe" });
  const { shop_id }: { shop_id: string } = await shop.json();
  await step("create-shop", shop);

  // 07: anonymous request is rejected
  const anon = await api("GET", "/shops");
  snapshot("shops-unauthorized", { status: anon.status(), body: await anon.json().catch(() => null) });
  expect(anon.status()).toBe(401);

  // 08: invite member
  const invite = await api("POST", `/shops/${shop_id}/invites`, token, {
    shop_id,
    email: memberEmail,
    role: "employee",
  });
  const { invite_id } = await step("invite-member", invite);

  // 09–10: member wallet auth, accepts invite
  const mauth = await apiAuth(api, member);
  const mtoken = mauth.token;
  const mme = await api("GET", "/auth/me", mtoken);
  await step("auth-member", mme);
  const accepted = await api("POST", `/invites/${invite_id}/accept`, mtoken, {});
  await step("accept-invite", accepted);

  // 11: employee may list goods…
  const listGoods = await api("GET", `/shops/${shop_id}/goods`, mtoken);
  await step("list-goods-employee", listGoods);

  // 12: …but not add them (403)
  const forbidden = await api("POST", `/shops/${shop_id}/goods`, mtoken, {
    shop_id,
    name: "x",
    price: "1",
  });
  snapshot("add-goods-forbidden", { status: forbidden.status(), body: await forbidden.json().catch(() => null) });
  expect(forbidden.status()).toBe(403);

  // 13: owner adds goods
  const goods = await api("POST", `/shops/${shop_id}/goods`, token, {
    shop_id,
    name: "espresso",
    price: "3.50",
  });
  const { goods_id } = await step("add-goods", goods);

  // 14: owner adds payment destination
  const dest = await api("POST", `/shops/${shop_id}/destinations`, token, {
    shop_id,
    chain: "solana",
    address: "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR",
  });
  const { destination_id } = await step("add-destination", dest);

  // 15: checkout → invoice
  const checkout = await api("POST", `/shops/${shop_id}/checkout`, token, {
    shop_id,
    goods_ids: [goods_id],
    destination_id,
  });
  const invoice = await step("checkout", checkout);
  expect(invoice.qr_payload).toContain("solana:");
  expect(invoice.qr_payload).toContain("amount=3.5");
  expect(invoice.total).toBe("3.5");

  // 16: invoice PDF
  const pdf = await request.fetch(`${BACKEND}${invoice.pdf_url}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  const bytes = await pdf.body();
  snapshot("invoice-pdf", {
    status: pdf.status(),
    contentType: pdf.headers()["content-type"],
    magic: Buffer.from(bytes.subarray(0, 4)).toString("ascii"),
    minSize: bytes.length > 1000,
  });
  expect(pdf.ok()).toBeTruthy();
  expect(pdf.headers()["content-type"]).toBe("application/pdf");
  expect(bytes.subarray(0, 4)).toEqual(Buffer.from("%PDF"));

  // 17: shop creation without a subscription succeeds (free), but the quota
  // still applies — the second shop is rejected.
  const noSub = await apiAuth(api, Keypair.generate());
  const freeShop = await api("POST", "/shops", noSub.token, { name: "No Sub Cafe" });
  expect(freeShop.status()).toBe(200);
  const blocked = await api("POST", "/shops", noSub.token, { name: "No Sub Cafe 2" });
  snapshot("create-shop-without-subscription", {
    status: blocked.status(),
    body: await blocked.json().catch(() => null),
  });
  expect(blocked.status()).toBe(400);
});
