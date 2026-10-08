import { defineConfig } from "@playwright/test";

const BACKEND = process.env.POS_BACKEND_URL ?? "http://localhost:18080";
const UI = process.env.POS_UI_URL ?? "http://localhost:8081";

export default defineConfig({
  testDir: "./specs",
  timeout: 30_000,
  retries: 1,
  // the phantom extension specs share one browser profile (SingletonLock) and
  // one backend — must not run in parallel
  workers: 1,
  use: {
    baseURL: UI,
  },
});
