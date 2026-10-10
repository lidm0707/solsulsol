const fs = require("fs");
const { Keypair, Connection, PublicKey } = require("@solana/web3.js");
const { createAssociatedTokenAccountIdempotent, transferChecked } = require("@solana/spl-token");
const bs58 = require("bs58").default;
const nacl = require("tweetnacl");
const RPC = "http://localhost:8899";
const API = "http://localhost:18787/api";
const env = Object.fromEntries(fs.readFileSync(".solana/env", "utf8").trim().split("\n").map(l => l.split("=")));
(async () => {
  const c = new Connection(RPC, "confirmed");
  const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(".solana/payer.json"))));
  const dest = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(".solana/recipient.json"))));
  const user = Keypair.generate();
  let r = await fetch(API + "/auth/nonce", {method:"POST", headers:{"Content-Type":"application/json"}, body: JSON.stringify({wallet: user.publicKey.toBase58()})});
  const n = await r.json();
  const sig = bs58.encode(nacl.sign.detached(Buffer.from(n.message, "utf8"), user.secretKey));
  r = await fetch(API + "/auth/verify", {method:"POST", headers:{"Content-Type":"application/json"}, body: JSON.stringify({wallet: user.publicKey.toBase58(), nonce: n.nonce, signature: sig})});
  const { token } = await r.json();
  const H = {"Content-Type":"application/json", Authorization: "Bearer " + token};
  r = await fetch(API + "/payments/orders", {method:"POST", headers: H, body: JSON.stringify({})});
  const order = await r.json();
  console.log("order:", JSON.stringify(order));
  const mint = new PublicKey(order.mint);
  const destAta = await createAssociatedTokenAccountIdempotent(c, payer, mint, dest.publicKey);
  const payerAta = await createAssociatedTokenAccountIdempotent(c, payer, mint, payer.publicKey);
  const tx = await transferChecked(c, payer, payerAta, mint, destAta, payer.publicKey, 1_000_000n, 6, [payer, dest]);
  console.log("paid:", tx);
  await c.confirmTransaction(tx, "finalized").catch(e => console.log("confirm warn:", e.message));
  for (let i = 0; i < 20; i++) {
    r = await fetch(API + "/payments/verify", {method:"POST", headers: H, body: JSON.stringify({order_id: order.order_id, signature: tx})});
    const b = await r.text();
    console.log("verify:", r.status, b.slice(0, 300));
    if (r.status === 200) break;
    await new Promise(rs => setTimeout(rs, 2000));
  }
})();
