import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

// worker under wrangler dev via docker/docker-compose.cloudflare-e2e.yml
const BACKEND = process.env.CF_BACKEND ?? "http://localhost:18787";
const SNAPDIR = path.join(__dirname, "..", "snapshots", "cloudflare_flow");
const CAPDIR = path.join(__dirname, "..", "captures", "cloudflare_flow");
const UPDATE = process.env.UPDATE_SNAPSHOTS === "1";

const rand = () => Math.random().toString(36).slice(2, 10);

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

function mask(value: unknown): unknown {
  const json = JSON.stringify(value, null, 2) ?? "";
  const masked = json
    .replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi, "<uuid>")
    .replace(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g, "<jwt>")
    .replace(/owner-[a-z0-9]+@/g, "owner-<rand>@")
    .replace(/member-[a-z0-9]+@/g, "member-<rand>@");
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

test("worker API flow: signup → shop → goods → checkout", async () => {
  const owner = { email: `owner-${rand()}@test.dev`, password: "hunter2234" };
  const member = { email: `member-${rand()}@test.dev`, password: "hunter2234" };

  const reg = await api("POST", "/auth/register", undefined, owner);
  snapshot("register-owner", { status: reg.status(), body: await reg.json().catch(() => null) });
  const login = await api("POST", "/auth/login", undefined, owner);
  const { token, user_id }: { token: string; user_id: string } = await login.json();
  await step("login-owner", login);
  expect(token).toBeTruthy();
  expect(user_id).toBeTruthy();

  const shop = await api("POST", "/shops", token, { name: "CF E2E Cafe" });
  const { shop_id }: { shop_id: string } = await shop.json();
  await step("create-shop", shop);

  const anon = await api("GET", "/shops");
  snapshot("shops-unauthorized", { status: anon.status(), body: await anon.json().catch(() => null) });
  expect(anon.status()).toBe(401);

  const invite = await api("POST", `/shops/${shop_id}/invites`, token, {
    shop_id,
    email: member.email,
    role: "employee",
  });
  const { invite_id } = await step("invite-member", invite);

  await api("POST", "/auth/register", undefined, member);
  const mlogin = await api("POST", "/auth/login", undefined, member);
  const { token: mtoken }: { token: string } = await mlogin.json();
  await step("login-member", mlogin);
  const accepted = await api("POST", `/invites/${invite_id}/accept`, mtoken, {
    password: member.password,
  });
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
