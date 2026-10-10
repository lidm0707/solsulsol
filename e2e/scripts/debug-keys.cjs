// Pinpoint which persisted keypair breaks createMint on surfpool offline.
const { Connection, Keypair, LAMPORTS_PER_SOL } = require("@solana/web3.js");
const { createMint } = require("@solana/spl-token");
const fs = require("node:fs");
const path = require("node:path");

const c = new Connection("http://localhost:8899", "confirmed");
const load = (n) =>
  Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(path.join(".solana", `${n}.json`), "utf8"))));

async function funded(name) {
  const kp = Keypair.generate();
  const sig = await c.requestAirdrop(kp.publicKey, LAMPORTS_PER_SOL);
  await c.confirmTransaction(sig, "confirmed");
  return kp;
}

(async () => {
  const persistedPayer = load("payer");
  const persistedMint = load("usdc-mint");
  console.log("persisted payer:", persistedPayer.publicKey.toBase58(), "bal:", await c.getBalance(persistedPayer.publicKey));
  console.log("persisted mint:", persistedMint.publicKey.toBase58(), "bal:", await c.getBalance(persistedMint.publicKey));

  // case 1: fresh payer + persisted mint
  try {
    const fp = await funded();
    const m = await createMint(c, fp, fp.publicKey, null, 6, persistedMint);
    console.log("case1 fresh-payer persisted-mint OK:", m.toBase58());
  } catch (e) {
    console.log("case1 FAILED:", e.message?.slice(0, 120));
  }

  // case 2: persisted payer + fresh mint
  try {
    const fm = Keypair.generate();
    const m = await createMint(c, persistedPayer, persistedPayer.publicKey, null, 6, fm);
    console.log("case2 persisted-payer fresh-mint OK:", m.toBase58());
  } catch (e) {
    console.log("case2 FAILED:", e.message?.slice(0, 120));
  }
})();
