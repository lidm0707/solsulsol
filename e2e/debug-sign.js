// debug: phantom signTransaction hang inspection (v2, verbose)
const { chromium } = require("@playwright/test");
const fs = require("fs");
const path = require("path");
const { Connection, PublicKey, SystemProgram, Transaction, LAMPORTS_PER_SOL } = require("@solana/web3.js");

const PHANTOM_DIR = path.join(__dirname, ".phantom");
const PASS = fs.readFileSync(path.join(PHANTOM_DIR, "test-wallet-password.txt"), "utf8").trim();
const SURFPOOL = "http://localhost:8899";
const WEB3 = path.join(__dirname, "node_modules", "@solana", "web3.js", "lib", "index.iife.min.js");
const log = (m) => console.error(new Date().toISOString().slice(11, 19), m);

(async () => {
  log("launching");
  const ctx = await chromium.launchPersistentContext(path.join(PHANTOM_DIR, "profile"), {
    channel: "chromium",
    headless: true,
    viewport: { width: 420, height: 760 },
    args: [
      `--disable-extensions-except=${path.join(PHANTOM_DIR, "ext")}`,
      `--load-extension=${path.join(PHANTOM_DIR, "ext")}`,
      "--headless=new",
    ],
  });
  log("launched");
  ctx.on("page", (p) => log("NEW PAGE: " + p.url()));
  let [w] = ctx.serviceWorkers();
  if (!w) w = await ctx.waitForEvent("serviceworker", { timeout: 30000 });
  const extId = new URL(w.url()).host;
  log("extId " + extId);

  const page = await ctx.newPage();
  await page.goto("http://localhost:8081/?api=http://localhost:18080");
  log("page loaded");
  await page.addScriptTag({ path: WEB3 });
  log("web3 injected");
  await page.evaluate(() => window.phantom.solana.connect().catch((e) => log("connect err " + e)));
  log("connect called");
  await page.waitForTimeout(3000);

  const conn = new Connection(SURFPOOL);
  const bh = await conn.getLatestBlockhash("finalized");
  const from = fs.readFileSync(path.join(PHANTOM_DIR, "test-wallet-address.txt"), "utf8").trim();
  log("blockhash " + bh.blockhash.slice(0, 10));

  const signing = page.evaluate(
    async ({ from, blockhash }) => {
      const w3 = window.solanaWeb3;
      const tx = new w3.Transaction({
        feePayer: new w3.PublicKey(from),
        recentBlockhash: blockhash,
      }).add(
        w3.SystemProgram.transfer({
          fromPubkey: new w3.PublicKey(from),
          toPubkey: new w3.PublicKey("4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR"),
          lamports: 3.5 * 1e9,
        }),
      );
      try {
        const res = await window.phantom.solana.signTransaction(tx);
        return { ok: true, sig: res.signature ? Buffer.from(res.signature).toString("base64") : null };
      } catch (e) {
        return { ok: false, err: String(e) };
      }
    },
    { from, blockhash: bh.blockhash },
  );

  const res = await Promise.race([
    signing,
    new Promise((resolve) => setTimeout(() => resolve({ ok: false, err: "SIGN TIMEOUT 25s" }), 30000)),
  ]);
  log("sign result: " + JSON.stringify(res).slice(0, 300));
  const notif = ctx.pages().find((p) => p.url().includes("notification.html"));
  if (notif) {
    await notif.waitForLoadState();
    await notif.waitForTimeout(1500);
    await notif.screenshot({ path: "captures/phantom_extension/notif.png" });
    const txt = await notif.evaluate(() => document.body.innerText);
    log("notif text: " + txt.slice(0, 300).replace(/\n+/g, " | "));
    const btns = await notif.evaluate(() =>
      Array.from(document.querySelectorAll("button"))
        .map((b) => b.textContent?.trim() || b.getAttribute("aria-label"))
        .filter(Boolean),
    );
    log("notif buttons: " + JSON.stringify(btns));
  } else {
    log("no notification page");
  }
  await ctx.close();
  process.exit(0);
})().catch((e) => {
  log("ERR " + e.message);
  process.exit(1);
});
