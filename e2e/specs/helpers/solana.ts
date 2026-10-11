// Shared Solana + wallet-auth helpers for e2e specs.
// Keys come from e2e/.solana/*.json (created by scripts/prepare-solana.cjs);
// the chain is the private surfpool at localhost:8899.
import fs from "node:fs";
import path from "node:path";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotent,
  createMint,
  createTransferCheckedInstruction,
  mintTo,
} from "@solana/spl-token";

export const SURFPOOL = process.env.SURFPOOL ?? "http://localhost:8899";
const SOLANA_DIR = path.join(__dirname, "..", "..", ".solana");
const AIRDROP_SOL = 5;
const SUB_USDC = 1_000_000n; // 1 USDC, 6 decimals

// tweetnacl has no bundled types in this repo; the require keeps TS quiet.
// eslint-disable-next-line @typescript-eslint/no-var-requires
const nacl = require("tweetnacl");
// eslint-disable-next-line @typescript-eslint/no-var-requires
const bs58 = require("bs58").default;

const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const SPL_TOKEN_PROGRAM = TOKEN_PROGRAM;
const MEMO_PROGRAM = new PublicKey("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
const ATA_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

export function loadKeypair(name: string): Keypair {
  const file = path.join(SOLANA_DIR, `${name}.json`);
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(file, "utf8"))));
}

export const usdcMint = () => loadKeypair("usdc-mint").publicKey;
export const payer = () => loadKeypair("payer");
export const recipient = () => loadKeypair("recipient");

/** Fresh throwaway wallet for per-run user identity (needs no funds;
 * subscription payments are still made by the shared payer). */
export const freshWallet = (): Keypair => Keypair.generate();

export function conn(): Connection {
  return new Connection(SURFPOOL, "confirmed");
}

async function ensureFunded(c: Connection, kp: Keypair) {
  const bal = await c.getBalance(kp.publicKey);
  if (bal < 0.5 * LAMPORTS_PER_SOL) {
    const sig = await c.requestAirdrop(kp.publicKey, AIRDROP_SOL * LAMPORTS_PER_SOL);
    await c.confirmTransaction(sig, "confirmed");
    // surfpool quirk: the confirmed airdrop can lag the debiting bank; wait
    // until the balance is actually visible before sending spending txs.
    for (let i = 0; i < 20; i++) {
      if ((await c.getBalance(kp.publicKey)) > bal) break;
      await new Promise((r) => setTimeout(r, 250));
    }
    await new Promise((r) => setTimeout(r, 500));
  }
}

function ataOf(owner: PublicKey, mint: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()],
    ATA_PROGRAM,
  )[0];
}

/** Creates the test USDC mint + funds the payer wallet. Idempotent. */
export async function ensureUsdc(c: Connection): Promise<PublicKey> {
  const mintKp = loadKeypair("usdc-mint");
  const p = payer();
  // NOTE: the mint keypair must NOT be airdropped — createAccount requires a
  // zero-lamports target; the payer funds the mint's rent.
  await ensureFunded(c, p);
  let mint = mintKp.publicKey;
  if (!(await c.getAccountInfo(mint))) {
    mint = await createMint(c, p, mintKp.publicKey, null, 6, mintKp);
  }
  const payerAta = await createAssociatedTokenAccountIdempotent(c, p, mint, p.publicKey);
  const bal = Number((await c.getTokenAccountBalance(payerAta)).value.amount);
  if (bal < 5_000_000) {
    await mintTo(c, p, mint, payerAta, mintKp, 1_000_000_000n);
  }
  return mint;
}

/** Pays one subscription: `minorUsdc` × 1e-6 USDC, payer → recipient.
 * A unique memo makes each transaction distinct — identical transfers inside
 * one blockhash window collide on the surfpool ("already been processed"). */
export async function paySubscriptionAmount(c: Connection, minorUsdc: bigint): Promise<string> {
  const p = payer();
  const mint = usdcMint();
  const dest = recipient();
  const destAta = await createAssociatedTokenAccountIdempotent(c, p, mint, dest.publicKey);
  const tx = new Transaction().add(
    new TransactionInstruction({
      programId: MEMO_PROGRAM,
      keys: [],
      data: Buffer.from(`e2e-${Date.now()}-${Math.random()}`),
    }),
    createTransferCheckedInstruction(
      ataOf(p.publicKey, mint),
      mint,
      destAta,
      p.publicKey,
      minorUsdc,
      6,
      [],
      SPL_TOKEN_PROGRAM,
    ),
  );
  tx.feePayer = p.publicKey;
  const { blockhash } = await c.getLatestBlockhash("confirmed");
  tx.recentBlockhash = blockhash;
  tx.sign(p);
  return c.sendRawTransaction(tx.serialize(), { skipPreflight: false });
}

/** Pays one $1 subscription: payer → configured recipient. Returns the signature. */
export async function paySubscription(c: Connection): Promise<string> {
  return paySubscriptionAmount(c, SUB_USDC);
}

/** Wallet sign-in over the API: nonce → sign the server's message → verify. */
export async function apiAuth(
  api: (method: string, p: string, token?: string, body?: unknown) => Promise<any>,
  wallet: Keypair,
): Promise<{ token: string; user_id: string }> {
  const nonceRes = await api("POST", "/auth/nonce", undefined, {
    wallet: wallet.publicKey.toBase58(),
  });
  if (nonceRes.status() >= 400) throw new Error(`nonce failed: ${nonceRes.status()}`);
  const { nonce, message } = await nonceRes.json();
  const verifyRes = await api("POST", "/auth/verify", undefined, {
    wallet: wallet.publicKey.toBase58(),
    nonce,
    signature: naclSign(message, wallet),
  });
  if (verifyRes.status() >= 400) throw new Error(`verify failed: ${verifyRes.status()}`);
  return verifyRes.json();
}

export function naclSign(message: string, wallet: Keypair): string {
  const sig: Uint8Array = nacl.sign.detached(
    Buffer.from(message, "utf8"),
    wallet.secretKey,
  );
  return bs58.encode(sig);
}

