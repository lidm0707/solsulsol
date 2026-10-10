// Isolate: createAccount + InitializeMint2 (what spl-token createMint does).
const { Connection, Keypair, Transaction, SystemProgram, LAMPORTS_PER_SOL, sendAndConfirmTransaction } = require("@solana/web3.js");
const { createInitializeMint2Instruction, getMintLen, MINT_SIZE, TOKEN_PROGRAM_ID } = require("@solana/spl-token");

const c = new Connection("http://localhost:8899", "confirmed");
(async () => {
  const payer = Keypair.generate();
  const mint = Keypair.generate();
  const sig = await c.requestAirdrop(payer.publicKey, LAMPORTS_PER_SOL);
  await c.confirmTransaction(sig, "confirmed");

  const lamports = await c.getMinimumBalanceForRentExemption(MINT_SIZE);
  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: mint.publicKey,
      space: MINT_SIZE,
      lamports,
      programId: TOKEN_PROGRAM_ID,
    }),
    createInitializeMint2Instruction(mint.publicKey, 6, payer.publicKey, payer.publicKey),
  );
  const s = await sendAndConfirmTransaction(c, tx, [payer, mint]);
  console.log("mint2 tx:", s);
  const info = await c.getAccountInfo(mint.publicKey);
  console.log("mint owner:", info?.owner?.toBase58(), "space:", info?.space);

  // now spl-token createMint itself, fresh keypair
  const { createMint } = require("@solana/spl-token");
  const kp = Keypair.generate();
  const m = await createMint(c, payer, payer.publicKey, null, 6, kp);
  console.log("spl createMint ok:", m.toBase58());
})().catch((e) => {
  console.error("FAILED:", e.message?.slice(0, 600));
  if (e.getLogs) console.error("logs:", e.getLogs());
  process.exit(1);
});
