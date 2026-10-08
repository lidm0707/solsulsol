//! phantom pays the pos qr: surfpool-funded wallet signs the exact amount/destination
//! from the qr payload, approval captured, transaction confirmed on private surfpool.

import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  clusterApiUrl,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";

const BACKEND = process.env.BACKEND ?? "http://localhost:18080";
const UI = process.env.UI ?? "http://localhost:8081";
const SURFPOOL = process.env.SURFPOOL ?? "http://localhost:8899";
const PHANTOM_DIR = path.join(__dirname, "..", ".phantom");
const EXT_DIR = path.join(PHANTOM_DIR, "ext");
const PROFILE_DIR = path.join(PHANTOM_DIR, "profile");
const CAPTURES = path.join(__dirname, "..", "captures", "phantom_pay");
const WEB3_BUNDLE = path.join(__dirname, "..", "node_modules", "@solana", "web3.js", "lib", "index.iife.min.js");
const PRICE_SOL = 3.5;
const AIRDROP_SOL = 10;

const rand = () => Math.random().toString(36).slice(2, 10);
const sleepMs = (ms: number) => new Promise((r) => setTimeout(r, ms));

const FLOW = "phantom pay: qr amount → phantom signs → confirmed on private surfpool";

test(`${FLOW}`, async ({ playwright }) => {
  test.setTimeout(300_000);
  fs.mkdirSync(CAPTURES, { recursive: true });
  const wallet = fs
    .readFileSync(path.join(PHANTOM_DIR, "test-wallet-address.txt"), "utf8")
    .trim();
  const pass = fs.readFileSync(path.join(PHANTOM_DIR, "test-wallet-password.txt"), "utf8").trim();
  expect(wallet, "run phantom_extension first to create the wallet").toMatch(/^[1-9A-HJ-NP-Za-km-z]{32,48}$/);

  const conn = new Connection(SURFPOOL);

  // fund the wallet on surfpool
  let balance = await conn.getBalance(new PublicKey(wallet));
  if (balance < PRICE_SOL * LAMPORTS_PER_SOL) {
    const sig = await conn.requestAirdrop(new PublicKey(wallet), AIRDROP_SOL * LAMPORTS_PER_SOL);
    await conn.confirmTransaction(sig, "confirmed");
    balance = await conn.getBalance(new PublicKey(wallet));
  }
  console.log(`[phantom-pay] wallet funded: ${balance / 1e9} SOL`);
  expect(balance).toBeGreaterThanOrEqual(PRICE_SOL * LAMPORTS_PER_SOL);

  const ctx = await playwright.chromium.launchPersistentContext(PROFILE_DIR, {
    channel: "chromium",
    headless: true,
    ignoreHTTPSErrors: true,
    viewport: { width: 420, height: 760 },
    args: [
      `--disable-extensions-except=${EXT_DIR}`,
      `--load-extension=${EXT_DIR}`,
      "--headless=new",
      // phantom (testnet mode) talks to api.devnet.solana.com — land it on private surfpool
      "--host-resolver-rules=MAP api.devnet.solana.com localhost:18443",
    ],
  });
  try {
    let [worker] = ctx.serviceWorkers();
    if (!worker) worker = await ctx.waitForEvent("serviceworker", { timeout: 30_000 });
    const extId = new URL(worker.url()).host;

    // ---- pos side: run a sale up to the pay screen -------------------------
    const pos = await ctx.newPage();
    await pos.goto(`${UI}/?api=${encodeURIComponent(BACKEND)}`);
    const email = `pay-${rand()}@test.dev`;
    await pos.locator('input[placeholder="email"]').fill(email);
    await pos.locator('input[placeholder="password"]').fill("hunter2234");
    await pos.getByRole("button", { name: "Register" }).click();
    await expect(pos.locator(".toast.success")).toContainText("registered");
    await pos.getByRole("button", { name: "Login", exact: true }).click();
    await expect(pos.locator('[aria-label="new-shop"]')).toBeVisible({ timeout: 15_000 });
    await pos.locator('[aria-label="new-shop"]').click();
    await pos.locator('input[placeholder="new shop name"]').fill("Pay Cafe");
    await pos.locator('[aria-label="create-shop"]').click();
    const chip = pos.locator(".shop-chip", { hasText: "Pay Cafe" });
    await expect(chip).toBeVisible({ timeout: 15_000 });
    await chip.click();
    await expect(pos.locator('[aria-label="tab-goods"]')).toBeVisible({ timeout: 15_000 });
    await pos.locator('[aria-label="tab-goods"]').click();
    await pos.locator('[aria-label="create-goods"]').click();
    await pos.locator('input[placeholder="name"]').fill("espresso");
    await pos.locator('input[placeholder="price"]').fill(String(PRICE_SOL));
    await pos.locator('input[placeholder="cost"]').fill("1");
    await pos.locator('input[placeholder="units"]').fill("10");
    await pos.locator('[aria-label="add-goods"]').click();
    await expect(pos.locator("tr", { hasText: "espresso" })).toBeVisible({ timeout: 15_000 });
    await pos.locator('[aria-label="tab-settings"]').click();
    await pos.locator('[aria-label="section-add-account"]').click();
    // shop receive address ≠ the paying wallet, so the transfer is observable
    await pos
      .locator('input[placeholder="address"]')
      .fill("4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR");
    await pos.locator('[aria-label="add-destination"]').click();
    await expect(pos.locator(".list-row", { hasText: "Solana" })).toBeVisible({ timeout: 15_000 });
    await pos.locator('[aria-label="tab-sell"]').click();
    const tile = pos.locator(".goods-tile", { hasText: "espresso" });
    await expect(tile).toBeVisible({ timeout: 15_000 });
    await tile.click();
    await pos.locator('[aria-label="new-doc"]').click();
    await expect(pos.locator(".qr svg").first()).toBeVisible({ timeout: 15_000 });
    await pos.screenshot({ path: path.join(CAPTURES, "1-pay-screen.png") });

    // qr payload from the api — amount + destination the wallet must honor
    const login = await (await fetch(`${BACKEND}/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password: "hunter2234" }),
    })).json();
    const shops = await (await fetch(`${BACKEND}/shops`, {
      headers: { Authorization: `Bearer ${login.token}` },
    })).json();
    const billings = await (await fetch(`${BACKEND}/shops/${shops[0].shop_id}/billings`, {
      headers: { Authorization: `Bearer ${login.token}` },
    })).json();
    const uri: string = billings[0].qr_payload;
    console.log(`[phantom-pay] qr payload: ${uri}`);
    const m = /^solana:([1-9A-HJ-NP-Za-km-z]{32,48})\?amount=([\d.]+)/.exec(uri);
    expect(m, `solana pay uri: ${uri}`).not.toBeNull();
    const to = m![1];
    const amountSol = Number(m![2]);
    expect(amountSol).toBe(PRICE_SOL);

    // ---- wallet side: phantom signs the payment ---------------------------
    const walletPage = await ctx.newPage();
    await walletPage.goto(`chrome-extension://${extId}/popup.html`);
    await walletPage.waitForTimeout(1_500);
    if (await walletPage.locator('input[type="password"]').first().isVisible().catch(() => false)) {
      await walletPage.locator('input[type="password"]').first().fill(pass);
      await walletPage.getByRole("button", { name: /unlock/i }).click();
      await walletPage.waitForTimeout(2_000);
    }

    // web3 in the page, provider connect (approval popup handled if it appears)
    await pos.addScriptTag({ path: WEB3_BUNDLE });
    const connecting = pos.evaluate(() =>
      ((window as any).phantom.solana as { connect: () => Promise<any> })
        .connect()
        .then((r: any) => ({ publicKey: String(r.publicKey) }))
        .catch((e: unknown) => ({ err: String(e) })),
    );
    const preApproval = await ctx.waitForEvent("page", { timeout: 8_000 }).catch(() => null);
    if (preApproval) {
      await preApproval.waitForLoadState();
      await preApproval.waitForTimeout(1_500);
      if (await preApproval.locator('input[type="password"]').first().isVisible().catch(() => false)) {
        await preApproval.locator('input[type="password"]').first().fill(pass);
        await preApproval.getByRole("button", { name: /unlock/i }).click();
        await preApproval.waitForTimeout(2_000);
      }
      await preApproval.screenshot({ path: path.join(CAPTURES, "2-phantom-approve-payment.png") });
      await preApproval.getByRole("button", { name: /approve|sign|confirm|connect/i }).first().click({ timeout: 30_000 });
    }
    const connected = await connecting;
    console.log(`[phantom-pay] provider connect: ${JSON.stringify(connected)}`);

    // build the transfer from the qr payload; latest blockhash from surfpool
    const bh = await conn.getLatestBlockhash("finalized");
    const signing = pos.evaluate(
      async ({ from, to, lamports, blockhash }) => {
        const w = (window as any).solanaWeb3;
        const tx = new w.Transaction({
          feePayer: new w.PublicKey(from),
          recentBlockhash: blockhash,
        }).add(
          w.SystemProgram.transfer({
            fromPubkey: new w.PublicKey(from),
            toPubkey: new w.PublicKey(to),
            lamports,
          }),
        );
        try {
          const res = await (window as any).phantom.solana.signTransaction(tx);
          return { ok: true, tx: res.serialize().toString("base64") };
        } catch (e) {
          return { ok: false, err: String(e) };
        }
      },
      { from: wallet, to, lamports: Math.round(amountSol * LAMPORTS_PER_SOL), blockhash: bh.blockhash },
    );

    // approval popup appears while signTransaction pends — capture it
    const approval = await ctx.waitForEvent("page", { timeout: 30_000 }).catch(() => null);
    expect(approval, "phantom approval popup must open").toBeTruthy();
    await approval!.waitForLoadState();
    await approval!.waitForTimeout(2_000);
    // the popup can show the lock screen — unlock in place, then approve
    if (await approval!.locator('input[type="password"]').first().isVisible().catch(() => false)) {
      await approval!.locator('input[type="password"]').first().fill(pass);
      await approval!.getByRole("button", { name: /unlock/i }).click();
      await approval!.waitForTimeout(2_000);
      console.log("[phantom-pay] popup unlocked");
    }

    // abnormal-UI detection: anything that signals phantom could not process
    // the request normally (e.g. simulation failure against its own rpc)
    const popupText = await approval!.evaluate(() => document.body.innerText);
    const abnormal = [
      /failed to simulate/i,
      /something went wrong/i,
      /unable to/i,
      /error occurred/i,
    ].some((re) => re.test(popupText));
    if (abnormal) {
      console.log("[phantom-pay] ABNORMAL UI on approval popup:");
      console.log(popupText.slice(0, 300).replace(/\n+/g, " | "));
      await approval!.screenshot({ path: path.join(CAPTURES, "ABNORMAL-ui.png") });
    }
    await approval!.screenshot({ path: path.join(CAPTURES, "2-phantom-approve-payment.png") });
    await approval!.getByRole("button", { name: /approve|sign|confirm|connect/i }).first().click({ timeout: 30_000 });
    console.log(`[phantom-pay] approval clicked (abnormalUi: ${abnormal})`);

    // abnormal flow: phantom escalates to an "Are you sure?" blocked-confirmation
    // screen with a risk checkbox — acknowledge and force-confirm
    if (abnormal) {
      await approval!.waitForTimeout(1_500);
      const checkbox = approval!.getByText(/I understand that I could lose/i).first();
      if (await checkbox.isVisible().catch(() => false)) {
        await checkbox.click();
        await approval!.waitForTimeout(800);
        const force = approval!.getByText(/Yes, confirm \(unsafe\)/i).first();
        await force.click({ timeout: 10_000 });
        console.log("[phantom-pay] unsafe-confirm acknowledged");
      }
    }

    const signed = (await Promise.race([signing, sleepMs(60_000).then(() => null)])) as
      | { ok: boolean; tx?: string; err?: string }
      | null;
    if (!signed) {
      await approval!.screenshot({ path: path.join(CAPTURES, "ABNORMAL-after-confirm.png") });
      await approval!.waitForTimeout(2_000);
      const afterText = await approval!.evaluate(() => document.body.innerText);
      console.log("[phantom-pay] popup after confirm:", afterText.slice(0, 300).replace(/\n+/g, " | "));
    }

    // failure reasoning: what happened and why, persisted next to the captures
    const reason = !signed
      ? "phantom never resolved signTransaction — likely blocked on the unsafe-confirm escalation or its own RPC cannot simulate a surfpool blockhash"
      : !signed.ok
        ? `phantom rejected signing: ${signed.err}`
        : null;
    if (reason) {
      fs.writeFileSync(
        path.join(CAPTURES, "fail-reason.json"),
        `${JSON.stringify(
          {
            abnormalUi: abnormal,
            abnormalPatterns: ["failed to simulate", "something went wrong", "unable to", "error occurred"].filter((p) =>
              new RegExp(p, "i").test(popupText),
            ),
            popupText: popupText.slice(0, 800),
            reason,
            note: "red banner is expected in this setup: phantom simulates against its own rpc, which does not know surfpool blockhashes. the tx itself is valid — we send it to surfpool ourselves",
          },
          null,
          2,
        )}\n`,
      );
      console.log(`[phantom-pay] FAIL REASON: ${reason}`);
    }
    expect(signed, "phantom did not resolve signTransaction in 60s").toBeTruthy();
    expect(signed!.ok, `sign failed: ${signed!.err ?? ""}`).toBe(true);
    const txBase64 = signed!.tx!;

    // send from node to private surfpool + confirm (patient: phantom is slow
    // through the abnormal-ui escalation, blockhash may age)
    const raw = Buffer.from(txBase64, "base64");
    const sig = await conn.sendRawTransaction(raw, { skipPreflight: false });
    const confirmed = await conn.confirmTransaction(sig, "confirmed").catch(() => null);
    let status = confirmed?.value.err === null;
    if (!status) {
      // landed late? check by signature instead of failing immediately
      await sleepMs(5_000);
      const statuses = await conn.getSignatureStatuses([sig]);
      const s = statuses.value[0];
      status = !!s && s.err === null && s.confirmationStatus !== null;
    }
    if (!status) {
      fs.writeFileSync(
        path.join(CAPTURES, "fail-reason.json"),
        `${JSON.stringify(
          {
            stage: "on-chain confirm",
            signature: sig,
            reason: "signed tx did not confirm on surfpool in time — blockhash likely expired during phantom's slow unsafe-confirm escalation",
            abnormalUi: abnormal,
          },
          null,
          2,
        )}\n`,
      );
    }
    expect(status, `transaction ${sig} did not confirm`).toBe(true);

    const paidTo = await conn.getBalance(new PublicKey(to));
    const walletAfter = await conn.getBalance(new PublicKey(wallet));
    console.log(`[phantom-pay] signature: ${sig}`);
    console.log(`[phantom-pay] destination balance: ${paidTo / 1e9} SOL, wallet left: ${walletAfter / 1e9} SOL`);
    expect(paidTo).toBeGreaterThanOrEqual(Math.round(amountSol * LAMPORTS_PER_SOL));

    fs.writeFileSync(
      path.join(CAPTURES, "pay-result.json"),
      `${JSON.stringify(
        {
          uri,
          amountSol,
          destination: to,
          signature: sig,
          confirmed: status,
          destinationBalance: paidTo / 1e9,
          walletBalanceAfter: walletAfter / 1e9,
          rpc: SURFPOOL,
          abnormalUi: abnormal,
          at: new Date().toISOString(),
        },
        null,
        2,
      )}\n`,
    );
    // surfpool proof: the confirmed transaction as the private chain sees it
    const txProof = await conn.getTransaction(sig, { maxSupportedTransactionVersion: 0 });
    fs.writeFileSync(
      path.join(CAPTURES, "surfpool-proof.json"),
      `${JSON.stringify(
        {
          signature: sig,
          slot: txProof?.slot,
          fee: txProof?.meta?.fee,
          status: txProof?.meta?.err === null ? { Ok: null } : txProof?.meta?.err,
          blockTime: txProof?.blockTime ? new Date(txProof.blockTime * 1000).toISOString() : null,
          rpc: SURFPOOL,
        },
        null,
        2,
      )}\n`,
    );

    // surfpool studio dashboard (visual proof of the chain state)
    try {
      const studio = await ctx.newPage();
      await studio.goto("http://localhost:18488", { timeout: 10_000 });
      await studio.waitForTimeout(3_000);
      await studio.screenshot({ path: path.join(CAPTURES, "4-surfpool-studio.png"), fullPage: true });
    } catch (e) {
      console.warn("surfpool studio not reachable, skipping studio capture:", String(e).slice(0, 120));
    }
    await pos.screenshot({ path: path.join(CAPTURES, "3-after-payment.png") });
  } finally {
    await ctx.close();
  }
});
