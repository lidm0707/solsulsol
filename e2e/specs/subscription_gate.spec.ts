import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { test, expect } from "@playwright/test";
import {
  conn,
  ensureUsdc,
  freshWallet,
  paySubscriptionAmount,
} from "./helpers/solana";
import { injectMockWallet, gotoUi, uiSignIn, saveVideo } from "./helpers/ui";

// Subscription gate, end to end with captures + video at every step:
//   unpaid → shop click opens the gate BEFORE the shop (no entry) →
//   pay in the gate → enter → near-expiry red banner (≤3 days) →
//   renew in Settings → grace warn banner → locked notice, still no entry.

const BACKEND = process.env.BACKEND ?? "http://host.docker.internal:18080";
const PG = "solsulsol-e2e-postgres-test-1";

let shotNo = 0;
async function shot(page: any, name: string) {
  shotNo += 1;
  const out = path.join(__dirname, "..", "captures", "subscription_gate");
  fs.mkdirSync(out, { recursive: true });
  await page.screenshot({
    path: path.join(out, `${String(shotNo).padStart(2, "0")}-${name}.png`),
    fullPage: true,
  });
}

/** Forces the wallet's subscription row into a lifecycle state. */
function sql(stmt: string) {
  execFileSync(
    "docker",
    ["exec", PG, "psql", "-U", "pos_test", "-d", "pos_test", "-c", stmt],
    { stdio: "pipe" },
  );
}

/** Reloads the UI and signs back in (the auth token is not persisted). */
async function reloadAndSignIn(page: any) {
  await page.reload();
  await page.locator('[aria-label="connect-wallet"]').first().click();
  await page.getByRole("button", { name: "Sign In" }).click();
  await expect(page.locator('[aria-label="new-shop"]')).toBeVisible({ timeout: 20_000 });
}

function forceSub(wallet: string, status: string, expiresSql: string, graceSql = "NULL") {
  sql(`INSERT INTO subscriptions (id, user_id, status, started_at, expires_at, grace_until)
       SELECT gen_random_uuid(), u.id, '${status}', now(), ${expiresSql}, ${graceSql}
       FROM users u WHERE u.wallet = '${wallet}'
       ON CONFLICT (user_id) DO UPDATE
       SET status = '${status}', expires_at = ${expiresSql}, grace_until = ${graceSql}`);
}

test.afterEach(async ({ page }) => {
  await saveVideo(page, "subscription_gate");
});

test("gate before shop: unpaid blocked → pay → near-expiry warn → renew → grace → locked", async ({
  page,
}) => {
  test.setTimeout(240_000);
  const wallet = freshWallet();
  await ensureUsdc(conn());
  await injectMockWallet(page, wallet);
  await gotoUi(page);
  await uiSignIn(page);

  // ---- unpaid: create a shop → the gate opens over the shop list, NO entry --
  await page.locator('[aria-label="new-shop"]').click();
  await page.locator('input[placeholder="new shop name"]').fill("Gate Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required", {
    timeout: 15_000,
  });
  await expect(page.locator(".tabbar")).toHaveCount(0);
  await shot(page, "gate-before-shop-no-entry");

  // ---- shop click while unpaid → gate again, still no entry -----------------
  await page.locator(".modal-close").click();
  await page.locator(".shop-chip", { hasText: "Gate Cafe" }).click();
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required", {
    timeout: 15_000,
  });
  await expect(page.locator(".tabbar")).toHaveCount(0);
  await shot(page, "shop-click-blocked-gate");

  // ---- pay inside the gate → auto-enter the pending shop --------------------
  await page.locator('[aria-label="subscribe"]').click();
  const sigInput = page.locator(
    'input[placeholder="paste transaction signature after paying"]',
  );
  await expect(sigInput).toBeVisible({ timeout: 15_000 });
  const signature = await paySubscriptionAmount(conn(), 1_000_000n);
  await sigInput.fill(signature);
  await page.getByRole("button", { name: "Verify payment" }).click();
  await expect(page.locator(".toast.success")).toContainText("Payment confirmed", {
    timeout: 30_000,
  });
  await expect(page.locator(".modal-overlay")).toHaveCount(0, { timeout: 20_000 });
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await shot(page, "paid-entered-shop");

  // ---- near expiry (1 day left): re-enter → red-bordered banner ------------
  forceSub(wallet.publicKey.toBase58(), "active", "now() + interval '1 day'");
  await reloadAndSignIn(page);
  await page.locator(".shop-chip", { hasText: "Gate Cafe" }).click();
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".banner.expiring")).toContainText("Subscription expires in", {
    timeout: 20_000,
  });
  await shot(page, "near-expiry-red-banner");

  // ---- renew via Settings → banner clears -----------------------------------
  await page.locator('[aria-label="tab-settings"]').click();
  await page.locator('[aria-label="group-subscription"]').click();
  await expect(page.locator('[aria-label="subscribe"]')).toBeVisible({ timeout: 15_000 });
  await page.locator('[aria-label="subscribe"]').click();
  const renewInput = page.locator(
    'input[placeholder="paste transaction signature after paying"]',
  );
  await expect(renewInput).toBeVisible({ timeout: 15_000 });
  const renewSig = await paySubscriptionAmount(conn(), 1_000_000n);
  await renewInput.fill(renewSig);
  await page.getByRole("button", { name: "Verify payment" }).click();
  await expect(page.locator(".toast.success")).toContainText("Payment confirmed", {
    timeout: 30_000,
  });
  await shot(page, "renewed-in-settings");

  // ---- grace (expired ≤7d): re-enter → orange warning banner ----------------
  forceSub(
    wallet.publicKey.toBase58(),
    "grace",
    "now() - interval '1 day'",
    "now() + interval '6 day'",
  );
  await reloadAndSignIn(page);
  await page.locator(".shop-chip", { hasText: "Gate Cafe" }).click();
  await expect(page.locator(".tabbar")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".banner.warn")).toContainText("renew within 7 days", {
    timeout: 20_000,
  });
  await shot(page, "grace-warning-banner");

  // ---- lapsed: locked — shop click is blocked with a locked notice ----------
  forceSub(
    wallet.publicKey.toBase58(),
    "lapsed",
    "now() - interval '8 day'",
    "now() - interval '1 day'",
  );
  await reloadAndSignIn(page);
  await page.locator(".shop-chip", { hasText: "Gate Cafe" }).click();
  await expect(page.locator(".toast.error")).toContainText("Shop locked", {
    timeout: 15_000,
  });
  await expect(page.locator(".modal-overlay")).toContainText("Subscription required");
  await expect(page.locator(".tabbar")).toHaveCount(0);
  await shot(page, "locked-shop-click-no-entry");
});
