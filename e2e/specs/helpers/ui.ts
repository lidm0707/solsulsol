// UI-side helpers: inject a deterministic mock Phantom provider, sign in
// through the real wasm UI, and drive the $1 subscription payment.
import fs from "node:fs";
import path from "node:path";
import type { Page } from "@playwright/test";
import { expect } from "@playwright/test";
import { Keypair, PublicKey } from "@solana/web3.js";
import { conn, paySubscription } from "./solana";

const NACL_BUNDLE = path.join(
  __dirname,
  "..",
  "..",
  "node_modules",
  "tweetnacl",
  "nacl-fast.min.js",
);
const UI = process.env.UI ?? "http://localhost:8081";
const BACKEND = process.env.BACKEND ?? "http://localhost:18080";

/** Injects window.solana (Phantom-compatible) signing with `wallet`. */
export async function injectMockWallet(page: Page, wallet: Keypair) {
  await page.addInitScript({ content: fs.readFileSync(NACL_BUNDLE, "utf8") });
  await page.addInitScript(
    ({ sk, pk }: { sk: number[]; pk: string }) => {
      const nacl = (window as any).nacl;
      const keypair = nacl.sign.keyPair.fromSecretKey(new Uint8Array(sk));
      const provider = {
        isPhantom: true,
        publicKey: { toString: () => pk },
        connect: async () => ({ publicKey: provider.publicKey }),
        disconnect: async () => {},
        signMessage: async (msg: Uint8Array) => ({
          signature: nacl.sign.detached(msg, keypair.secretKey),
          publicKey: provider.publicKey,
        }),
      };
      (window as any).solana = provider;
    },
    {
      sk: Array.from(wallet.secretKey),
      pk: wallet.publicKey.toBase58(),
    },
  );
}

export async function gotoUi(page: Page) {
  await page.goto(`${UI}/?api=${encodeURIComponent(BACKEND)}`);
  await expect(page.locator("h1")).toHaveText("solsulsol");
}

/** Connect (mock Phantom) → Sign In → lands on the shop-select dashboard. */
export async function uiSignIn(page: Page) {
  await page.locator('[aria-label="connect-wallet"]').first().click();
  await page.getByRole("button", { name: "Sign In" }).click();
  await expect(page.locator('[aria-label="new-shop"]')).toBeVisible({ timeout: 20_000 });
}

/** Subscribes through the open gate modal: subscribe → pay $1 → verify. */
export async function uiSubscribe(page: Page) {
  await page.locator('[aria-label="subscribe"]').click();
  const sigInput = page.locator('input[placeholder="paste transaction signature after paying"]');
  await expect(sigInput).toBeVisible({ timeout: 15_000 });
  const signature = await paySubscription(conn());
  await sigInput.fill(signature);
  await page.getByRole("button", { name: "Verify payment" }).click();
  await expect(page.locator(".toast.success")).toContainText("Payment confirmed", {
    timeout: 30_000,
  });
}

/** Closes the page (flushes the recording) and saves the video as videos/<name>.webm. */
export async function saveVideo(page: Page, name: string) {
  const video = page.video();
  if (!video) return;
  const out = path.join(__dirname, "..", "..", "videos");
  fs.mkdirSync(out, { recursive: true });
  await page.context().close();
  await video.saveAs(path.join(out, `${name}.webm`));
}

export function ensurePublicKey(s: string): PublicKey {
  return new PublicKey(s);
}
