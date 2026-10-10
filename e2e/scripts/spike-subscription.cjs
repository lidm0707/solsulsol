// Spike: validates the full subscription path against the real e2e stack.
// auth (nonce/verify) → order → on-chain USDC transfer on surfpool → verify.
const assert = require("node:assert");
const {
  Connection,
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
} = require("@solana/web3.js");
const {
  createAssociatedTokenAccountIdempotent,
  createMint,
  mintTo,
  transferChecked,
} = require("@solana/spl-token");
const nacl = require("tweetnacl");
const bs58 = require("bs58").default;
const fs = require("node:fs");
const path = require("node:path");

const BACKEND = "http://localhost:18080";
const SURFPOOL = "http://localhost:8899";
const SOLANA_DIR = path.join(__dirname, "..", ".solana");
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

function loadKeypair(name) {
  return Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(path.join(SOLANA_DIR, `${name}.json`), "utf8"))),
  );
}

function b58(bytes) {
  return bs58.encode(bytes);
}

async function api(method, p, token, body) {
  const res = await fetch(`${BACKEND}${p}`, {
    method,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let json = null;
  try {
    json = JSON.parse(text);
  } catch {
    console.log(`[api] non-json response from ${method} ${p}: status=${res.status} body=${text.slice(0, 200)}`);
  }
  return { status: res.status, json };
}

async function main() {
  const c = new Connection(SURFPOOL, "confirmed");
  const mintKp = loadKeypair("usdc-mint");
  const payer = loadKeypair("payer");
  const recipient = loadKeypair("recipient");
  const ataOf = (owner, mint) =>
    PublicKey.findProgramAddressSync(
      [owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()],
      ATA_PROGRAM,
    )[0];

  for (const kp of [payer]) {
    const bal = await c.getBalance(kp.publicKey);
    if (bal < 0.5 * LAMPORTS_PER_SOL) {
      const sig = await c.requestAirdrop(kp.publicKey, 5 * LAMPORTS_PER_SOL);
      await c.confirmTransaction(sig, "confirmed");
      // surfpool quirk: wait until the credit is visible in the bank.
      for (let i = 0; i < 20; i++) {
        if ((await c.getBalance(kp.publicKey)) > bal) break;
        await new Promise((r) => setTimeout(r, 250));
      }
      await new Promise((r) => setTimeout(r, 500));
    }
    const after = await c.getBalance(kp.publicKey);
    console.log("payer funded:", after);
    if (after < 0.5 * LAMPORTS_PER_SOL) throw new Error("payer not funded");
  }

  let mint = mintKp.publicKey;
  if (!(await c.getAccountInfo(mint))) {
    mint = await createMint(c, payer, mintKp.publicKey, null, 6, mintKp);
  }
  console.log("mint:", mint.toBase58());
  const payerAta = await createAssociatedTokenAccountIdempotent(c, payer, mint, payer.publicKey);
  const bal = Number((await c.getTokenAccountBalance(payerAta)).value.amount);
  if (bal < 5_000_000) {
    await mintTo(c, payer, mint, payerAta, mintKp, 1_000_000_000n);
  }
  console.log("payer ata balance:", (await c.getTokenAccountBalance(payerAta)).value.uiAmount);

  // ---- wallet auth ----
  const wallet = payer;
  const n = await api("POST", "/auth/nonce", undefined, { wallet: wallet.publicKey.toBase58() });
  assert.equal(n.status, 200, JSON.stringify(n.json));
  const sig = b58(nacl.sign.detached(Buffer.from(n.json.message, "utf8"), wallet.secretKey));
  const v = await api("POST", "/auth/verify", undefined, {
    wallet: wallet.publicKey.toBase58(),
    nonce: n.json.nonce,
    signature: sig,
  });
  assert.equal(v.status, 200, JSON.stringify(v.json));
  const token = v.json.token;
  console.log("auth ok, user:", v.json.user_id);

  // shop without subscription must 403
  const noShop = await api("POST", "/shops", token, { name: "too early" });
  console.log("shop w/o sub:", noShop.status, JSON.stringify(noShop.json));
  assert.equal(noShop.status, 403);

  // ---- order + on-chain payment + verify ----
  const o = await api("POST", "/payments/orders", token, {});
  assert.equal(o.status, 200, JSON.stringify(o.json));
  console.log("order:", JSON.stringify(o.json));
  assert.equal(o.json.recipient, recipient.publicKey.toBase58(), "recipient mismatch");
  assert.equal(o.json.mint, mint.toBase58(), "mint mismatch");

  const destAta = await createAssociatedTokenAccountIdempotent(
    c, payer, mint, recipient.publicKey,
  );
  const tx = await transferChecked(
    c, payer, ataOf(payer.publicKey, mint), mint, destAta, payer.publicKey,
    1_000_000n, 6, [payer, recipient],
  );
  console.log("paid:", tx);

  const ver = await api("POST", "/payments/verify", token, {
    order_id: o.json.order_id,
    signature: tx,
  });
  console.log("verify:", ver.status, JSON.stringify(ver.json));
  assert.equal(ver.status, 200);
  assert.equal(ver.json.status, "active");

  // duplicate verify must fail
  const dup = await api("POST", "/payments/verify", token, {
    order_id: o.json.order_id,
    signature: tx,
  });
  console.log("dup verify:", dup.status, JSON.stringify(dup.json));
  assert.equal(dup.status, 409);

  // now shop creation works
  const shop = await api("POST", "/shops", token, { name: "Spike Cafe" });
  assert.equal(shop.status, 200, JSON.stringify(shop.json));
  console.log("shop created:", shop.json.shop_id);
  console.log("SPIKE OK");
}

main().catch((e) => {
  console.error("SPIKE FAILED:", e.message);
  process.exit(1);
});
