// Captures admin routes as a super-user, using a token seeded directly into
// localStorage (admin_auth_token). Requires the API to be running with
// --dev-auth-user <id> so that token is accepted without a real session.
//
// Uses a location named "Test Unit" so no real unit/member data ever
// appears in a screenshot. Change LOCATION_NAME if your test data differs.
import { chromium } from "playwright";

const outDir = process.argv[2];
if (!outDir) {
  console.error("usage: node capture-admin.mjs <output-dir>");
  process.exit(1);
}

const BASE_URL = "http://localhost:5173";
const LOCATION_NAME = "Test Unit";

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });

await page.goto(`${BASE_URL}/admin`);
await page.evaluate(() => localStorage.setItem("admin_auth_token", "devtoken"));
await page.goto(`${BASE_URL}/admin`);
await page.waitForTimeout(2000);
await page.screenshot({ path: `${outDir}/admin-location-selector.png` });
console.log("saved admin-location-selector");

await page.getByText(LOCATION_NAME, { exact: false }).first().click();
await page.waitForTimeout(1500);
await page.screenshot({ path: `${outDir}/admin-dashboard.png` });
console.log("saved admin-dashboard");

const routes = [
  ["members", "admin-members-list"],
  ["sessions", "admin-sessions-list"],
  ["reports", "admin-reports"],
  ["settings", "admin-settings"],
  ["activity", "admin-activity"],
  ["locations", "admin-locations"],
  ["users", "admin-users"],
  ["categories", "admin-categories"],
];
for (const [path, name] of routes) {
  await page.goto(`${BASE_URL}/admin/${path}`);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${outDir}/${name}.png` });
  console.log("saved", name);
}

// Member edit form, desktop + mobile widths.
await page.goto(`${BASE_URL}/admin/members`);
await page.waitForTimeout(1500);
const editLink = page.getByRole("link", { name: "Edit" }).first();
if (await editLink.count()) {
  await editLink.click();
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${outDir}/admin-member-edit.png` });
  console.log("saved admin-member-edit");

  await page.setViewportSize({ width: 375, height: 800 });
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${outDir}/admin-member-edit-375.png` });
  console.log("saved admin-member-edit-375");
}

await browser.close();
