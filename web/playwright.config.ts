// Browser tests of the web app in demo mode (no server needed): every page must
// update without a reload (Kees's standing rule).
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "tests",
  timeout: 20_000,
  retries: 0,
  reporter: [["list"]],
  use: { baseURL: "http://localhost:5173", trace: "off" },
  // A fake microphone (and no permission prompt) and audio that may play without a click,
  // for the voice tests.
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], launchOptions: { args: [
    "--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream", "--autoplay-policy=no-user-gesture-required",
  ] } } }],
  webServer: { command: "node build.mjs --serve", url: "http://localhost:5173", reuseExistingServer: false, timeout: 60_000 },
});
