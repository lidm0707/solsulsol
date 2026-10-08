// dig phantom mmkv state for solana RPC endpoints
const { chromium } = require("@playwright/test");
const path = require("path");
const PHANTOM_DIR = path.join(__dirname, ".phantom");

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
  let [w] = ctx.serviceWorkers();
  if (!w) w = await ctx.waitForEvent("serviceworker", { timeout: 30000 });
  const v = await w.evaluate(async () => {
    const all = await chrome.storage.local.get(["mmkv:state:flags", "mmkv:items:flags", "mmkv:items:localStorage"]);
    return JSON.stringify(all);
  });
  const re = /https?:\/\/[^"\\, ]+/g;
  const urls = [...new Set(v.match(re) ?? [])];
  console.log("total urls:", urls.length);
  console.log(urls.join("\n"));
  await ctx.close();
})().catch((e) => console.error(e.message));
