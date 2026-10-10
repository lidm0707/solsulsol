import { test, expect } from "@playwright/test";
import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { conn, ensureUsdc } from "./helpers/solana";
import { uiSubscribe } from "./helpers/ui";

const BACKEND = process.env.BACKEND ?? "http://localhost:18080";
const UI = process.env.UI ?? "http://localhost:8081";
const PHANTOM_DIR = path.join(__dirname, "..", ".phantom");
const EXT_DIR = path.join(PHANTOM_DIR, "ext");
const PROFILE_DIR = path.join(PHANTOM_DIR, "profile");
const CAPTURES = path.join(__dirname, "..", "captures", "phantom_extension");

const rand = () => Math.random().toString(36).slice(2, 10);
const ADDRESS_FALLBACK = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";

const FLOW = "phantom ext: real chrome + phantom extension → create test wallet → injected in pos ui";

test(`${FLOW}`, async ({ playwright }) => {
  test.setTimeout(240_000);
  fs.mkdirSync(CAPTURES, { recursive: true });
  // the phantom profile wallet is reused across runs; drop its old account so
  // shop/subscription limits start clean (cascades to shops and orders)
  execSync(
    `docker exec solsulsol-e2e-postgres-test-1 psql -U pos_test -d pos_test -c 'TRUNCATE users CASCADE'`,
  );

  const ctx = await playwright.chromium.launchPersistentContext(PROFILE_DIR, {
    channel: "chromium",
    headless: true,
    viewport: { width: 420, height: 760 },
    args: [
      `--disable-extensions-except=${EXT_DIR}`,
      `--load-extension=${EXT_DIR}`,
      "--headless=new",
    ],
  });
  try {
    // extension id from its mv3 service worker
    let [worker] = ctx.serviceWorkers();
    if (!worker) worker = await ctx.waitForEvent("serviceworker", { timeout: 30_000 });
    const extId = new URL(worker.url()).host;
    console.log(`[phantom-ext] id ${extId}`);

    const extPage = await ctx.newPage();
    await extPage.goto(`chrome-extension://${extId}/popup.html`);

    // a fresh profile has no wallet; an existing one shows the unlock screen.
    // key off the persisted password file so we never re-run onboarding and
    // silently create another account
    const hasPassword = fs.existsSync(path.join(PHANTOM_DIR, "test-wallet-password.txt"));
    const needsWallet = !hasPassword;

    if (needsWallet) {
      await extPage.goto(`chrome-extension://${extId}/onboarding.html`);
      await extPage.waitForTimeout(3_000);
      await extPage.screenshot({ path: path.join(CAPTURES, "step-0-onboarding.png") });
      console.log(`[phantom-ext] onboarding text: ${(await extPage.evaluate(() => document.body.innerText)).slice(0, 600).replace(/\n/g, " | ")}`);
      await extPage.getByText("Create a New Wallet", { exact: true }).first().click({ timeout: 20_000 });
      const page = () => onboardingPage(ctx, extId);
      await page().waitForTimeout(1_500);
      await page().screenshot({ path: path.join(CAPTURES, "step-1-wallet-options.png") });

      // choose recovery-phrase wallet
      await page().getByText("Create a Recovery Phrase Wallet", { exact: true }).first().click({ timeout: 20_000 });
      await page().waitForTimeout(1_500);
      await page().screenshot({ path: path.join(CAPTURES, "step-2-created-click.png") });

      // agree to terms if shown
      const agree = page().getByRole("button", { name: /i agree|agree and continue/i }).first();
      if (await agree.isVisible({ timeout: 4_000 }).catch(() => false)) await agree.click();

      // password + terms of service, then Continue
      const pass = `Ph-${rand()}9A`;
      fs.writeFileSync(path.join(PHANTOM_DIR, "test-wallet-password.txt"), `${pass}\n`);
      const pw = page().locator('input[type="password"]');
      await pw.first().fill(pass, { timeout: 20_000 });
      if ((await pw.count()) > 1) await pw.nth(1).fill(pass);
      await page().locator("input[type=checkbox]").first().check({ timeout: 10_000 }).catch(() => {});
      await page().screenshot({ path: path.join(CAPTURES, "step-3-password.png") });
      await page().getByRole("button", { name: /continue/i }).first().click({ timeout: 10_000 });

      // secret recovery phrase — save it so the test wallet can be funded later
      await page().waitForTimeout(2_500);
      await page().screenshot({ path: path.join(CAPTURES, "step-4-seed.png") });
      const seedText = await page().evaluate(() => document.body.innerText);
      const phrase = extractPhrase(seedText);
      if (phrase) fs.writeFileSync(path.join(PHANTOM_DIR, "test-wallet-seed.txt"), `${phrase.join(" ")}\n`);
      console.log(`[phantom-ext] seed captured: ${phrase ? phrase.length : 0} words`);

      const saved = page().getByText(/saved|wrote down|keep it safe/i).first();
      if (await saved.isVisible({ timeout: 4_000 }).catch(() => false)) await saved.click();
      const next = page().getByRole("button", { name: /continue|next|show phrase/i }).first();
      if (await next.isVisible({ timeout: 4_000 }).catch(() => false)) await next.click();

      // dismiss any final promo screens
      for (let i = 0; i < 4; i++) {
        const done = page().getByRole("button", { name: /done|get started|close|explore|finish/i }).first();
        if (await done.isVisible({ timeout: 3_000 }).catch(() => false)) {
          await done.click();
          break;
        }
      }
      await page().waitForTimeout(1_500);
      await page().screenshot({ path: path.join(CAPTURES, "step-4-wallet-created.png") });
    } else {
      console.log("[phantom-ext] wallet already exists in profile");
    }

    await extPage.goto(`chrome-extension://${extId}/popup.html`);
    await extPage.waitForTimeout(1_500);

    // unlock an existing wallet
    if (!needsWallet && (await extPage.locator('input[type="password"]').first().isVisible().catch(() => false))) {
      const savedPass = fs.readFileSync(path.join(PHANTOM_DIR, "test-wallet-password.txt"), "utf8").trim();
      await extPage.locator('input[type="password"]').first().fill(savedPass);
      await extPage.getByRole("button", { name: /unlock/i }).click();
      await extPage.waitForTimeout(2_000);
      console.log("[phantom-ext] wallet unlocked");
    }
    await extPage.screenshot({ path: path.join(CAPTURES, "wallet-home.png") });

    // receive screen → Solana row → full address on the detail screen
    await extPage.getByText("Receive").first().click({ timeout: 15_000 });
    await extPage.waitForTimeout(2_000);
    await extPage.getByText("Solana", { exact: true }).first().click({ timeout: 15_000 });
    await extPage.waitForTimeout(2_500);
    await extPage.screenshot({ path: path.join(CAPTURES, "step-5-receive.png") });
    const text = (await extPage.evaluate(() => document.body.innerText)).replace(/\n/g, "");
    let addr = text.match(/\b[1-9A-HJ-NP-Za-km-z]{43,48}\b/)?.[0];
    if (!addr) {
      // full address may live outside innerText — take it from the clipboard via Copy
      await extPage.context().grantPermissions(["clipboard-read", "clipboard-write"]);
      await extPage.getByText("Copy", { exact: true }).first().click({ timeout: 10_000 });
      await extPage.waitForTimeout(1_000);
      addr = await extPage.evaluate(() => navigator.clipboard.readText());
    }
    if (addr && /^[1-9A-HJ-NP-Za-km-z]{32,48}$/.test(addr)) {
      fs.writeFileSync(path.join(PHANTOM_DIR, "test-wallet-address.txt"), `${addr}\n`);
    }
    console.log(`[phantom-ext] wallet address: ${addr ?? "not found"}`);

    // the real check: phantom injects its provider into the POS ui page
    const pos = await ctx.newPage();
    await pos.goto(`${UI}/?api=${encodeURIComponent(BACKEND)}`);
    await expect(pos.locator("h1")).toHaveText("solsulsol");
    const injected = await pos.evaluate(() => ({
      phantom: typeof (window as any).phantom !== "undefined",
      solana: typeof (window as any).phantom?.solana !== "undefined",
      isPhantom: (window as any).phantom?.solana?.isPhantom ?? false,
    }));
    console.log(`[phantom-ext] provider in pos page: ${JSON.stringify(injected)}`);
    expect(injected.phantom).toBe(true);
    expect(injected.solana).toBe(true);
    expect(injected.isPhantom).toBe(true);

    fs.writeFileSync(
      path.join(CAPTURES, "wallet-check.json"),
      `${JSON.stringify({ extensionId: extId, injectedInPosUi: injected, at: new Date().toISOString() }, null, 2)}\n`,
    );
    await pos.screenshot({ path: path.join(CAPTURES, "pos-with-phantom.png") });

    // phantom on the pay screen: sign in with the REAL phantom extension,
    // subscribe, then run a sale and capture the approval
    await ensureUsdc(conn());

    // connect: the extension opens an approval popup — capture it, approve
    const connectClick = pos.locator('[aria-label="connect-wallet"]').first().click();
    const connectPopup = await ctx.waitForEvent("page", { timeout: 15_000 }).catch(() => null);
    if (connectPopup) {
      await connectPopup.waitForLoadState();
      await connectPopup.waitForTimeout(2_000);
      await connectPopup.screenshot({ path: path.join(CAPTURES, "phantom-connect-popup.png") });
      await connectPopup.getByRole("button", { name: /connect/i }).first().click({ timeout: 20_000 });
    } else {
      console.log("[phantom-ext] no connect popup (already trusted)");
    }
    await connectClick;

    // signMessage: another popup approval
    await pos.getByRole("button", { name: "Sign In" }).click();
    const signPopup = await ctx.waitForEvent("page", { timeout: 20_000 }).catch(() => null);
    if (signPopup) {
      await signPopup.waitForLoadState();
      await signPopup.waitForTimeout(2_000);
      await signPopup.screenshot({ path: path.join(CAPTURES, "phantom-signin-popup.png") });
      await signPopup.getByRole("button", { name: /sign|approve|confirm/i }).first().click({ timeout: 20_000 });
    }
    await expect(pos.locator('[aria-label="new-shop"]')).toBeVisible({ timeout: 20_000 });

    // $1/mo subscription — no tabbar before a shop exists; the subscription
    // payment comes from the surfpool test payer (extension wallet secret is
    // locked, and it has no USDC anyway)
    await pos.locator('[aria-label="new-shop"]').click();
    await expect(pos.locator(".modal-overlay")).toContainText("Subscription required", { timeout: 15_000 });
    await uiSubscribe(pos);

    await expect(pos.locator('input[placeholder="new shop name"]')).toBeVisible({ timeout: 20_000 });
    await pos.locator('input[placeholder="new shop name"]').fill("Ext Cafe");
    await pos.locator('[aria-label="create-shop"]').click();
    const chip = pos.locator(".shop-chip", { hasText: "Ext Cafe" });
    await expect(chip).toBeVisible({ timeout: 15_000 });
    await chip.click();
    await expect(pos.locator('[aria-label="tab-goods"]')).toBeVisible({ timeout: 15_000 });
    await pos.locator('[aria-label="tab-goods"]').click();
    await pos.locator('[aria-label="create-goods"]').click();
    await pos.locator('input[placeholder="name"]').fill("espresso");
    await pos.locator('input[placeholder="price"]').fill("3.5");
    await pos.locator('input[placeholder="cost"]').fill("1.2");
    await pos.locator('input[placeholder="units"]').fill("10");
    await pos.locator('[aria-label="add-goods"]').click();
    await expect(pos.locator("tr", { hasText: "espresso" })).toBeVisible({ timeout: 15_000 });
    await pos.locator('[aria-label="tab-settings"]').click();
    await pos.locator('[aria-label="section-add-account"]').click();
    await pos.locator('input[placeholder="address"]').fill(addr ?? ADDRESS_FALLBACK);
    await pos.locator('[aria-label="add-destination"]').click();
    await expect(pos.locator(".list-row", { hasText: "Solana" })).toBeVisible({ timeout: 15_000 });
    await pos.locator('[aria-label="tab-sell"]').click();
    const tile = pos.locator(".goods-tile", { hasText: "espresso" });
    await expect(tile).toBeVisible({ timeout: 15_000 });
    await tile.click();
    await expect(pos.locator(".total-bar")).toContainText("3.5");
    await pos.locator('[aria-label="new-doc"]').click();
    await expect(pos.locator('[aria-label="pay-method"]').first()).toBeVisible({ timeout: 15_000 });
    await pos.locator('[aria-label="pay-method"]').first().click();
    await expect(pos.locator(".qr svg").first()).toBeVisible({ timeout: 15_000 });
    await pos.screenshot({ path: path.join(CAPTURES, "pay-screen-before-connect.png") });

    // connect → phantom opens its own approval window — capture it, then approve
    const connecting = pos.evaluate(
      () =>
        new Promise((resolve, reject) => {
          (window as any).phantom.solana
            .connect()
            .then((r: any) => resolve({ publicKey: String(r.publicKey) }))
            .catch(reject);
        }),
    );
    // first connect opens an approval popup; re-connects resolve without one
    const approval = await ctx
      .waitForEvent("page", { timeout: 8_000 })
      .catch(() => null);
    if (approval) {
      await approval.waitForLoadState();
      await approval.waitForTimeout(2_000);
      await approval.screenshot({ path: path.join(CAPTURES, "phantom-approval-popup.png") });
      await approval.getByRole("button", { name: /connect/i }).first().click({ timeout: 20_000 });
    } else {
      console.log("[phantom-ext] already connected — no approval popup");
    }
    const { publicKey } = (await connecting) as { publicKey: string };
    console.log(`[phantom-ext] connected publicKey: ${publicKey}`);
    expect(publicKey).toBe(addr);

    await pos.waitForTimeout(1_500);
    await pos.screenshot({ path: path.join(CAPTURES, "pay-screen-phantom-connected.png") });
    fs.writeFileSync(
      path.join(CAPTURES, "pay-connect.json"),
      `${JSON.stringify({ wallet: publicKey, payScreen: "qr visible", connected: true }, null, 2)}\n`,
    );
  } finally {
    await ctx.close();
  }
});

/** Pull the 12/24 seed words out of the onboarding screen text. */
function extractPhrase(text: string): string[] | null {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => /^[a-z]{3,8}$/.test(l));
  for (const size of [24, 12]) {
    if (lines.length >= size) {
      const seq = lines.slice(0, size);
      if (new Set(seq).size === size) return seq;
    }
  }
  return null;
}

/** Phantom sometimes moves onboarding to a new window/tab — find wherever it lives. */
function onboardingPage(ctx: any, extId: string): any {
  const pages = ctx.pages().filter((p: any) => p.url().includes(extId));
  return pages[pages.length - 1] ?? ctx.pages()[0];
}
