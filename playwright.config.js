// End-to-end smoke tests for the static site (e2e/). They run against `wrangler dev` on port
// 8787, serving dist/, so build first: `npm test` does both.
const { defineConfig, devices } = require("@playwright/test");

module.exports = defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  reporter: "list",
  use: { baseURL: "http://localhost:8787", ...devices["Desktop Chrome"] },
  webServer: {
    command: "npm run dev",
    url: "http://localhost:8787",
    reuseExistingServer: true,
    timeout: 60_000,
  },
});
