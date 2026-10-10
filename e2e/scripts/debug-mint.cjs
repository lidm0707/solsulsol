// Isolate createMint on surfpool offline.
const { Connection, Keypair, Transaction, SystemProgram, LAMPORTS_PER_SOL, sendAndConfirmTransaction } = require("@solana/web3.js");
const { createInitializeMintInstruction, getMintLen, MINT_SIZE, TOKEN_PROGRAM_ID } = require("@solana/spl-token");

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
    createInitializeMintInstruction(mint.publicKey, 6, payer.publicKey, payer.publicKey),
  );
  const s = await sendAndConfirmTransaction(c, tx, [payer, mint]);
  console.log("mint tx:", s);
  const info = await c.getAccountInfo(mint.publicKey);
  console.log("mint owner:", info?.owner?.toBase58(), "space:", info?.space, "lamports:", info?.lamports);
  const g = await c.getTransaction(s, { commitment: "confirmed" });
  console.log("meta.err:", g?.meta?.err, "logCount:", g?.meta?.logMessages?.length);
  console.log("logs:", JSON.stringify(g?.meta?.logMessages)?.slice(0, 500));
})().catch((e) => {
  console.error("FAILED:", e.message?.slice(0, 500));
  process.exit(1);
});
