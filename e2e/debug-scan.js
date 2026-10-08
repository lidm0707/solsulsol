const { chromium } = require("@playwright/test");

const UI = process.env.UI ?? "http://localhost:8081";
const BACKEND = process.env.BACKEND ?? "http://localhost:18080";
const rand = () => Math.random().toString(36).slice(2, 10);

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  page.on("console", (m) => console.log("[console]", m.type(), m.text()));
  page.on("pageerror", (e) => console.log("[pageerror]", e.message));

  const email = `dbg-${rand()}@test.dev`;
  await page.goto(`${UI}/?api=${encodeURIComponent(BACKEND)}`);
  await page.locator('input[placeholder="email"]').fill(email);
  await page.locator('input[placeholder="password"]').fill("hunter2234");
  await page.getByRole("button", { name: "Register" }).click();
  await page.locator(".toast.success").waitFor();
  await page.getByRole("button", { name: "Login", exact: true }).click();
  await page.locator('[aria-label="new-shop"]').waitFor();
  await page.locator('[aria-label="new-shop"]').click();
  await page.locator('input[placeholder="new shop name"]').fill("Dbg Cafe");
  await page.locator('[aria-label="create-shop"]').click();
  await page.locator(".shop-chip", { hasText: "Dbg Cafe" }).waitFor();
  await page.locator(".shop-chip", { hasText: "Dbg Cafe" }).click();
  await page.locator('[aria-label="tab-goods"]').click();
  await page.locator('[aria-label="create-goods"]').click();
  await page.locator('input[placeholder="name"]').fill("espresso");
  await page.locator('input[placeholder="price"]').fill("3.5");
  await page.locator('input[placeholder="units"]').fill("10");
  await page.locator('input[placeholder="barcode (optional)"]').fill("4006381333931");
  await page.locator('[aria-label="add-goods"]').click();
  await page.locator("tr", { hasText: "espresso" }).waitFor();

  await page.locator('[aria-label="tab-sell"]').click();
  const scan = page.locator('input[placeholder="scan or type barcode, Enter to add"]');
  await scan.waitFor();
  await scan.click();
  await page.keyboard.type("4006381333931");
  console.log("value before Enter:", await scan.inputValue());
  await scan.press("Enter");
  await page.waitForTimeout(1500);
  const toasts = await page.locator(".toast").allTextContents();
  console.log("toasts after Enter:", JSON.stringify(toasts));
  const classes = await page.locator(".goods-tile").evaluateAll((els) => els.map((e) => e.className));
  console.log("tile classes:", JSON.stringify(classes));

  await browser.close();
})().catch((e) => { console.error(e); process.exit(1); });
