// dump phantom service-worker chrome.storage to find network/RPC config
const { chromium } = require("@playwright/test");
const fs = require("fs");
const path = require("path");

const PHANTOM_DIR = path.join(__dirname, ".phantom");
const PASS = fs.readFileSync(path.join(PHANTOM_DIR, "test-wallet-password.txt"), "utf8").trim();

(async () => {
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
  let [worker] = ctx.serviceWorkers();
  if (!worker) worker = await ctx.waitForEvent("serviceworker", { timeout: 30000 });
  await page_setup(worker);

  async function page_setup(w) {
    const keys = await w.evaluate(async () => {
      const all = await chrome.storage.local.get(null);
      return Object.keys(all);
    });
    console.log("storage keys:", JSON.stringify(keys, null, 0));
    // look for anything network/cluster/rpc related
    for (const k of keys) {
      if (/network|cluster|rpc|chain|endpoint/i.test(k)) {
        const v = await w.evaluate(async (key) => await chrome.storage.local.get(key), k);
        console.log(`-- ${k}:`, JSON.stringify(v).slice(0, 800));
      }
    }
    const all = await w.evaluate(async () => await chrome.storage.local.get(null));
    const flat = JSON.stringify(all);
    for (const m of flat.matchAll(/https:\/\/[a-z.]*solana[a-z.]*\.[a-z]+/gi)) {
      console.log("rpc url found:", m[0]);
    }
  }
  await ctx.close();
})();
