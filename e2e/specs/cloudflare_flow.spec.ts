import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { Keypair } from "@solana/web3.js";
import { apiAuth, conn, ensureUsdc, freshWallet, paySubscription } from "./helpers/solana";

// worker under wrangler dev via docker/docker-compose.cloudflare-e2e.yml
const BACKEND = process.env.CF_BACKEND ?? "http://localhost:18787";
const SNAPDIR = path.join(__dirname, "..", "snapshots", "cloudflare_flow");
const CAPDIR = path.join(__dirname, "..", "captures", "cloudflare_flow");
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

const BASE58 = /[1-9A-HJ-NP-Za-km-z]{32,48}/g;

function maskString(key: string, value: string): string {
  if (key === "signature") return "<txsig>";
  if (key === "expires_at") return "<expires_at>";
  if (key === "nonce") return "<nonce>";
  // nonce + expiry are embedded in the signed message string
  return value
    .replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi, "<uuid>")
    .replace(/\d{4}-\d{2}-\d{2}T[\d:.]+(\+00:00|Z)/g, "<ts>")
    .replace(BASE58, "<wallet>");
}

function mask(value: unknown): unknown {
  const walk = (v: unknown, key = ""): unknown => {
    if (typeof v === "string") return maskString(key, v);
    if (Array.isArray(v)) return v.map((x) => walk(x));
    if (v && typeof v === "object") {
      return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x, k)]));
    }
    return v;
  };
  return walk(value);
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

async function capture(name: string, body: unknown, res: any) {
  const file = path.join(CAPDIR, `${name}.json`);
  fs.mkdirSync(CAPDIR, { recursive: true });
  fs.writeFileSync(
    file,
    JSON.stringify(
      {
        status: res.status(),
        contentType: res.headers()["content-type"] ?? null,
        body,
      },
      null,
      2,
    ) + "\n",
  );
}

test.beforeEach(({ request: r }) => {
  request = r;
  stepNo = 0;
  failures = [];
});

test.afterEach(() => {
  expect(failures, "snapshot mismatches").toEqual([]);
});

test("worker API flow: wallet auth → subscribe → shop → goods → checkout", async () => {
  await ensureUsdc(conn());
  const owner = freshWallet();
  // member wallet is fresh per run; its address is masked in snapshots anyway
  const member = Keypair.generate();

  const nonce = await api("POST", "/auth/nonce", undefined, {
    wallet: owner.publicKey.toBase58(),
  });
  await step("auth-nonce-owner", nonce);
  const { token, user_id }: { token: string; user_id: string } = await apiAuth(api, owner);
  snapshot("auth-verify-owner", {
    status: 200,
    body: { token: "<jwt>", user_id, wallet: "<wallet>" },
  });
  expect(token).toBeTruthy();
  expect(user_id).toBeTruthy();

  // $1/mo USDC subscription is required before shop creation
  const order = await api("POST", "/payments/orders", token, {});
  const orderBody = await step("create-order", order);
  const signature = await paySubscription(conn());
  const verified = await api("POST", "/payments/verify", token, {
    order_id: orderBody.order_id,
    signature,
  });
  const verifyBody = await step("verify-payment", verified);
  expect(verifyBody.status).toBe("active");

  const shop = await api("POST", "/shops", token, { name: "CF E2E Cafe" });
  const { shop_id }: { shop_id: string } = await shop.json();
  await step("create-shop", shop);

  const anon = await api("GET", "/shops");
  snapshot("shops-unauthorized", { status: anon.status(), body: await anon.json().catch(() => null) });
  expect(anon.status()).toBe(401);

  const invite = await api("POST", `/shops/${shop_id}/invites`, token, {
    shop_id,
    email: "member@cf-e2e.test",
    role: "employee",
  });
  const { invite_id } = await step("invite-member", invite);

  const mnonce = await api("POST", "/auth/nonce", undefined, {
    wallet: member.publicKey.toBase58(),
  });
  await step("auth-nonce-member", mnonce);
  const { token: mtoken } = await apiAuth(api, member);
  const accepted = await api("POST", `/invites/${invite_id}/accept`, mtoken, {});
  await step("accept-invite", accepted);

  const forbidden = await api("POST", `/shops/${shop_id}/goods`, mtoken, {
    shop_id,
    name: "x",
    price: "1",
  });
  snapshot("add-goods-forbidden", { status: forbidden.status(), body: await forbidden.json().catch(() => null) });
  expect(forbidden.status()).toBe(403);

  const goods = await api("POST", `/shops/${shop_id}/goods`, token, {
    shop_id,
    name: "espresso",
    price: "3.50",
  });
  const { goods_id } = await step("add-goods", goods);

  const dest = await api("POST", `/shops/${shop_id}/destinations`, token, {
    shop_id,
    chain: "solana",
    address: "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR",
  });
  const { destination_id } = await step("add-destination", dest);

  const checkout = await api("POST", `/shops/${shop_id}/checkout`, token, {
    shop_id,
    goods_ids: [goods_id],
    destination_id,
  });
  const invoice = await step("checkout", checkout);
  expect(invoice.qr_payload).toContain("solana:");
  expect(invoice.total).toBe("3.5");

  const logout = await api("POST", "/auth/logout", token, {});
  await step("logout-owner", logout);
  expect(logout.status()).toBeLessThan(400);
});

test("worker captures: health, ui assets", async ({ page }) => {
  const health = await api("GET", "/health");
  await capture("health", await health.json().catch(() => null), health);
  expect(health.ok()).toBeTruthy();

  // the worker serves the wasm UI via its [assets] binding
  fs.mkdirSync(CAPDIR, { recursive: true });
  const res = await page.goto(`${BACKEND}/?api=${BACKEND}`, { waitUntil: "domcontentloaded" });
  expect(res?.ok()).toBeTruthy();
  await page.waitForTimeout(5_000); // wasm boot
  await page.screenshot({ path: path.join(CAPDIR, "ui-landing.png"), fullPage: true });
  fs.writeFileSync(
    path.join(CAPDIR, "ui-landing.json"),
    JSON.stringify({ status: res?.status(), title: await page.title(), url: page.url() }, null, 2) + "\n",
  );
});
