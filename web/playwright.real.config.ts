// Viewer tests against a real Kompanion server with the real asset library (not run in
// CI: it has no library). KK_REAL_URL and KK_SESSION point at a test server.
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "tests-real",
  timeout: 60_000,
  retries: 0,
  reporter: [["list"]],
  use: { baseURL: process.env.KK_REAL_URL, trace: "off", launchOptions: { args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1400, height: 900 } } }],
});
