// verify phantom provider can talk to surfpool via request interception
const { chromium } = require("@playwright/test");
const fs = require("fs");
const path = require("path");

const PHANTOM_DIR = path.join(__dirname, ".phantom");
const PASS = fs.readFileSync(path.join(PHANTOM_DIR, "test-wallet-password.txt"), "utf8").trim();
const SURFPOOL = "http://localhost:8899";

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
  // phantom testnet mode talks to api.devnet.solana.com — redirect to private surfpool
  await ctx.route("**api.devnet.solana.com/**", async (route) => {
    const res = await fetch(SURFPOOL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: route.request().postData() ?? "{}",
    });
    route.fulfill({ response: res, body: await res.text() });
  });

  let [worker] = ctx.serviceWorkers();
  if (!worker) worker = await ctx.waitForEvent("serviceworker", { timeout: 30000 });
  const extId = new URL(worker.url()).host;
  const page = await ctx.newPage();
  await page.goto(`chrome-extension://${extId}/popup.html`);
  await page.waitForTimeout(2000);
  if (await page.locator('input[type="password"]').first().isVisible().catch(() => false)) {
    await page.locator('input[type="password"]').first().fill(PASS);
    await page.getByRole("button", { name: /unlock/i }).click();
    await page.waitForTimeout(2500);
  }
  await page.screenshot({ path: "captures/phantom_extension/explore-rpc-home.png" });
  const text = await page.evaluate(() => document.body.innerText);
  console.log("home:", text.slice(0, 200).replace(/\n+/g, " | "));
  await ctx.close();
})();
