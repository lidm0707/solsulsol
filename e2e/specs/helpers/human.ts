// Human-paced interaction helpers: realistic delays, char-by-char typing,
// mouse movement before clicks. For demo videos, not for fast CI specs.
import type { Page, Locator } from "@playwright/test";
import { expect } from "@playwright/test";

const BASE_DELAY_MS = Number(process.env.HUMAN_SPEED ?? 450);
const TYPE_DELAY_MS = Number(process.env.HUMAN_TYPE_SPEED ?? 110);

export const pause = (page: Page, ms?: number) =>
  page.waitForTimeout(ms ?? BASE_DELAY_MS);

export async function step(page: Page, name: string) {
  console.log(`[human] ${name}`);
  await pause(page);
}

export async function humanType(page: Page, locator: Locator, text: string) {
  await locator.click();
  await pause(page, TYPE_DELAY_MS * 2);
  await locator.pressSequentially(text, { delay: TYPE_DELAY_MS });
  await pause(page, TYPE_DELAY_MS * 2);
}

export async function humanClick(page: Page, locator: Locator) {
  await locator.hover();
  await pause(page, TYPE_DELAY_MS);
  await locator.click();
  await pause(page);
}

export async function expectVisible(locator: Locator, timeout = 15_000) {
  await expect(locator.first()).toBeVisible({ timeout });
}
