// Prepares deterministic Solana test keys for e2e: usdc-mint, payer, recipient.
// Writes e2e/.solana/<name>.json keypairs + e2e/.solana/env (compose env_file).
// Chain-dependent steps (mint creation, airdrops) happen in the specs via
// specs/helpers/solana.ts — this script only needs local key material.
const fs = require("node:fs");
const path = require("node:path");
const { Keypair } = require("@solana/web3.js");

const DIR = path.join(__dirname, "..", ".solana");

function loadOrCreate(name) {
  const file = path.join(DIR, `${name}.json`);
  if (fs.existsSync(file)) {
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(file, "utf8"))));
  }
  const kp = Keypair.generate();
  fs.writeFileSync(file, JSON.stringify(Array.from(kp.secretKey)));
  return kp;
}

function ensureKeys() {
  fs.mkdirSync(DIR, { recursive: true });
  const mint = loadOrCreate("usdc-mint");
  const payer = loadOrCreate("payer");
  const recipient = loadOrCreate("recipient");
  const env = [
    `USDC_MINT=${mint.publicKey.toBase58()}`,
    `PAYMENT_RECIPIENT=${recipient.publicKey.toBase58()}`,
    `SOLANA_RPC_URL=http://surfpool:8899`,
    `POS_DOMAIN=localhost`,
    `NONCE_TTL_SECS=300`,
    "",
  ].join("\n");
  fs.writeFileSync(path.join(DIR, "env"), env);
  console.log(`[solana-e2e] keys ready in ${DIR}`);
  console.log(`  mint=${mint.publicKey.toBase58()} recipient=${recipient.publicKey.toBase58()}`);
}

switch (process.argv[2] ?? "ensure-keys") {
  case "ensure-keys":
    ensureKeys();
    break;
  default:
    console.error("usage: node prepare-solana.cjs [ensure-keys]");
    process.exit(1);
}
