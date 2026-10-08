// dump RPC URLs from phantom's react-query cache
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
    const all = await chrome.storage.local.get("REACT_QUERY_OFFLINE_CACHE");
    return JSON.stringify(all["REACT_QUERY_OFFLINE_CACHE"]);
  });
  const re = /https?:\/\/[^"\\]+/g;
  const matches = [...new Set(v.match(re) ?? [])];
  console.log("all urls:", matches.length);
  console.log(matches.slice(0, 40).join("\n"));
  await ctx.close();
})().catch((e) => console.error(e.message));
