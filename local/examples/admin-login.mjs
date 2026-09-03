/**
 * Leave a browser on the admin dashboard, as a chosen user with a chosen location selected.
 *
 * Three things stand between a fresh browser profile and a usable admin page, and each
 * one fails in a way that doesn't obviously point at itself:
 *   1. The SPA decides whether to show the login page by looking for `admin_auth_token`
 *      in localStorage — a client-side test that never asks the server. With nothing
 *      stored you get the login form even though every request behind it would work.
 *   2. A passkey enrolment prompt covers the page on a profile that hasn't dismissed it,
 *      so the next click lands on the overlay instead of the app.
 *   3. With no location chosen the app shows the location picker instead of the page you
 *      asked for.
 *
 *   node local/examples/admin-login.mjs
 *   USER_EMAIL=testunit@seslogin.test node local/examples/admin-login.mjs
 *   GOTO=/admin/members node local/examples/admin-login.mjs
 *
 * Environment:
 *   USER_EMAIL     seeded user's email (default super@seslogin.test). Not `USER`, which
 *                  every shell already sets to your login name.
 *   TOKEN          use this token instead of a seeded one — for a token you issued
 *   LOCATION       location name to select (default "Test A Unit")
 *   GOTO           path to end on (default /admin)
 *   BASE_URL       web origin (default http://localhost:5173)
 *   HEADED=1       show the browser
 *   KEEP_OPEN=1    leave it open when the script finishes
 *   CHROMIUM_PATH  use an existing Chromium instead of Playwright's
 */
import {
  BASE_URL,
  USER_TOKENS,
  launch,
  finish,
  settle,
  requireStack,
} from "./fixtures.mjs";

const USER = process.env.USER_EMAIL ?? "super@seslogin.test";
const LOCATION = process.env.LOCATION ?? "Test A Unit";
const GOTO = process.env.GOTO ?? "/admin";

const token = process.env.TOKEN ?? USER_TOKENS[USER];
if (!token) {
  console.error(
    `No seeded token for ${USER}. Known users: ${Object.keys(USER_TOKENS).join(", ")}.\n` +
      `Pass TOKEN=... to use one you issued yourself.`,
  );
  process.exit(1);
}

await requireStack();
const { browser, page } = await launch();

// 1. Log in. Must be on the app's origin before writing its localStorage.
await page.goto(`${BASE_URL}/admin`);
await page.evaluate((t) => localStorage.setItem("admin_auth_token", t), token);
await page.goto(`${BASE_URL}/admin`, { waitUntil: "networkidle" });

// 2. Dismiss the passkey prompt if this profile hasn't seen it. Not always present, so
//    check rather than assume.
const later = page.getByRole("button", { name: /maybe later/i });
if (await later.count()) {
  await later.click();
  await page.waitForLoadState("networkidle");
}

// 3. Choose the location, if the picker is up. `exact` matters: the seeded unit names are
//    chosen so neither contains the other, but a substring match would be a silent
//    wrong-unit bug rather than an error.
const picker = page.getByText(LOCATION, { exact: true });
if (await picker.count()) {
  await picker.first().click();
  await page.waitForLoadState("networkidle");
}

if (GOTO !== "/admin") {
  await page.goto(`${BASE_URL}${GOTO}`, { waitUntil: "networkidle" });
}
await settle(page);

const body = await page.locator("body").innerText();
if (/please sign in|send code/i.test(body)) {
  console.error(
    `Still on the login page — the token was rejected. Re-run \`make local-seed\`, ` +
      `or check the API is running the local database.`,
  );
  await finish(browser, page, "/tmp/admin-login-failed.png");
  process.exit(1);
}

console.log(`Signed in as ${USER} at ${LOCATION}`);
console.log(`  now at ${page.url()}`);
console.log(`\n${body.slice(0, 600)}`);

await finish(browser, page, "/tmp/admin-login.png");
