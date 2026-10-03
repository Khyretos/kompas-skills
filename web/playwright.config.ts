// Browser tests of the web app in demo mode (no server needed): every page must
// update without a reload (Kees's standing rule).
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "tests",
  timeout: 20_000,
  retries: 0,
  reporter: [["list"]],
  use: { baseURL: "http://localhost:5173", trace: "off" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: { command: "node build.mjs --serve", url: "http://localhost:5173", reuseExistingServer: false, timeout: 60_000 },
});
