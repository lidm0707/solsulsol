// explore phantom settings UI to find the custom-RPC / testnet-mode path
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
  const extId = new URL(worker.url()).host;
  const page = await ctx.newPage();
  await page.goto(`chrome-extension://${extId}/popup.html`);
  await page.waitForTimeout(2000);

  const dump = async (tag) => {
    await page.screenshot({ path: `captures/phantom_extension/explore-${tag}.png` });
    const text = await page.evaluate(() => document.body.innerText);
    console.log(`--- ${tag} ---`);
    console.log(text.slice(0, 600).replace(/\n+/g, " | "));
    console.log();
  };

  if (await page.locator('input[type="password"]').first().isVisible().catch(() => false)) {
    await page.locator('input[type="password"]').first().fill(PASS);
    await page.getByRole("button", { name: /unlock/i }).click();
    await page.waitForTimeout(2000);
  }
  await dump("home");

  // open account menu → settings gear
  await page.getByText("Account 1").first().click().catch(() => {});
  await page.waitForTimeout(1500);
  await page.getByText("More").first().click().catch(() => {});
  await page.waitForTimeout(1500);
  await page.getByText("Wallet Settings").first().click().catch(() => {});
  await page.waitForTimeout(1500);
  await dump("wallet-settings");
  await page.getByText(/Developer/i).first().click().catch(() => {});
  await page.waitForTimeout(1500);
  await dump("developer-settings");

  // enable Testnet Mode — click the row text (the toggle itself swallows aria-clicks)
  await page.getByText("Testnet Mode", { exact: true }).first().click().catch(() => {});
  await page.waitForTimeout(2500);
  await dump("testnet-toggled");

  // find custom rpc: back to settings → Active Networks / network settings
  await page.getByText("More").first().click().catch(() => {});
  await page.waitForTimeout(1500);
  await page.getByText("Wallet Settings").first().click().catch(() => {});
  await page.waitForTimeout(1500);
  await page.getByText("Active Networks", { exact: false }).first().click().catch(() => {});
  await page.waitForTimeout(1000);
  // back — instead open Developer Settings → Testnet Mode row for network options
  await page.goBack().catch(() => {});
  await page.waitForTimeout(1000);
  await page.getByText("Developer Settings").first().click().catch(() => {});
  await page.waitForTimeout(1500);
  await page.getByText("Testnet Mode", { exact: true }).first().click().catch(() => {});
  await page.waitForTimeout(2000);
  await dump("testnet-options");

  // look for any element with aria-label or text about settings
  const labels = await page.evaluate(() =>
    Array.from(document.querySelectorAll('button, [role="button"], [aria-label]'))
      .map((el) => el.getAttribute("aria-label") || el.textContent?.trim())
      .filter((t) => t && t.length < 40)
      .slice(0, 40),
  );
  console.log("clickable labels:", JSON.stringify(labels));
  await ctx.close();
})();
