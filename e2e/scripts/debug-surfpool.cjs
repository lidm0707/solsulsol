// Debug surfpool tx processing: airdrop, system transfer, createAccount.
const { Connection, Keypair, PublicKey, LAMPORTS_PER_SOL, SystemProgram, Transaction, sendAndConfirmTransaction } = require("@solana/web3.js");

const c = new Connection("http://localhost:8899", "confirmed");
const kp = Keypair.generate();

(async () => {
  const sig = await c.requestAirdrop(kp.publicKey, 2 * LAMPORTS_PER_SOL);
  await c.confirmTransaction(sig, "confirmed");
  console.log("airdrop ok, balance:", await c.getBalance(kp.publicKey));

  // simple transfer with rent-exempt minimum for the recipient
  const rent = await c.getMinimumBalanceForRentExemption(0);
  console.log("rent-exempt(0):", rent);
  const t1 = new Transaction().add(
    SystemProgram.transfer({ fromPubkey: kp.publicKey, toPubkey: Keypair.generate().publicKey, lamports: rent }),
  );
  const s1 = await sendAndConfirmTransaction(c, t1, [kp]);
  console.log("transfer sig:", s1);
  const g1 = await c.getTransaction(s1, { commitment: "confirmed" });
  console.log("transfer tx meta.err:", g1?.meta?.err, "blockhash:", g1?.transaction?.message?.recentBlockhash?.slice(0, 12));

  // createAccount exactly like createMint does: space 82, mint-rent lamports
  const acc = Keypair.generate();
  const lam = await c.getMinimumBalanceForRentExemption(82);
  const t2 = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: kp.publicKey,
      newAccountPubkey: acc.publicKey,
      lamports: lam,
      space: 82,
      programId: new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
    }),
  );
  const s2 = await sendAndConfirmTransaction(c, t2, [kp, acc]);
  console.log("createAccount sig:", s2);
  const info = await c.getAccountInfo(acc.publicKey);
  console.log("created account owner:", info?.owner?.toBase58(), "space:", info?.space, "lamports:", info?.lamports);
  const g2 = await c.getTransaction(s2, { commitment: "confirmed" });
  console.log("createAccount meta.err:", g2?.meta?.err);
})().catch((e) => {
  console.error("DEBUG FAILED:", e.message?.slice(0, 400));
  process.exit(1);
});
