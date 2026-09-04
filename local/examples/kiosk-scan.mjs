/**
 * Leave a browser on the kiosk scan screen, enrolled as the seeded key kiosk.
 *
 * A kiosk normally reaches that screen by enrolling: an admin approves its public key, or
 * someone types a 6-digit code. Both are fine to test, and neither is what you want as a
 * *precondition* for testing a sign-in. So this installs the seeded keypair directly.
 *
 * How it works — the two halves the app looks for:
 *   1. IndexedDB `seslogin-kiosk` / store `keys` / key `<profile>` holds a CryptoKeyPair.
 *      The app signs each request with it (`SLKey` scheme, see api/src/session_key.rs).
 *      The real kiosk generates a non-extractable key and never exports it; we *import*
 *      the seeded one, also non-extractable, which is indistinguishable to the app.
 *   2. localStorage `kiosk_<profile>` records `authMode: "key"`, which is what makes
 *      KioskEnvironment start in the authed-key state instead of the code entry screen.
 *
 * The matching public key is on session TestAKiosk02 at Test A Unit in synthetic.json.
 *
 *   node local/examples/kiosk-scan.mjs
 *
 * Environment:
 *   PROFILE        kiosk profile, i.e. /kiosk/<profile> (default: the seed's, "default")
 *   BASE_URL       web origin (default http://localhost:5173)
 *   HEADED=1       show the browser
 *   KEEP_OPEN=1    leave it open when the script finishes
 *   CHROMIUM_PATH  use an existing Chromium instead of Playwright's
 */
import {
  BASE_URL,
  kioskKey,
  launch,
  finish,
  settleKiosk,
  requireStack,
} from "./fixtures.mjs";

const PROFILE = process.env.PROFILE ?? kioskKey.profile;

await requireStack();
const { browser, page } = await launch();

// Must be on the app's origin before touching its IndexedDB and localStorage.
await page.goto(`${BASE_URL}/kiosk/${PROFILE}`);

await page.evaluate(
  async ({ profile, privateKeyB64, publicKeyB64 }) => {
    const bytes = (b64) =>
      Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)).buffer;
    const algorithm = { name: "ECDSA", namedCurve: "P-256" };

    // Imported non-extractable, exactly as the kiosk generates its own: the app only
    // ever signs with it, and this way the fixture key can't leak back out of a browser
    // it was installed into.
    const privateKey = await crypto.subtle.importKey(
      "pkcs8",
      bytes(privateKeyB64),
      algorithm,
      false,
      ["sign"],
    );
    // The public half must stay exportable — the app re-derives the fingerprint from its
    // SPKI on every load, and a non-extractable public key would break that.
    const publicKey = await crypto.subtle.importKey(
      "spki",
      bytes(publicKeyB64),
      algorithm,
      true,
      ["verify"],
    );

    await new Promise((resolve, reject) => {
      const req = indexedDB.open("seslogin-kiosk", 1);
      req.onupgradeneeded = () => req.result.createObjectStore("keys");
      req.onerror = () => reject(req.error);
      req.onsuccess = () => {
        const db = req.result;
        const tx = db.transaction("keys", "readwrite");
        // CryptoKeys are structured-cloneable, so the pair persists without the private
        // key material ever being readable.
        tx.objectStore("keys").put({ privateKey, publicKey }, profile);
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      };
    });

    const key = `kiosk_${profile}`;
    const settings = JSON.parse(localStorage.getItem(key) ?? "{}");
    localStorage.setItem(
      key,
      // Clear any JWT from a previous code enrolment: a stored token wins over authMode
      // and would put the kiosk back on the legacy path.
      JSON.stringify({
        ...settings,
        authMode: "key",
        scanAuthToken: null,
        scanAuthTokenIssuedAt: null,
      }),
    );
  },
  {
    profile: PROFILE,
    privateKeyB64: kioskKey.private_key_pkcs8_b64,
    publicKeyB64: kioskKey.public_key_spki_b64,
  },
);

await page.goto(`${BASE_URL}/kiosk/${PROFILE}`, { waitUntil: "networkidle" });

// The signing key is loaded from IndexedDB before the first request can be signed, so the
// kiosk renders its loading state first. Wait that out rather than guessing a delay.
await settleKiosk(page);

const body = await page.locator("body").innerText();
if (/enter the .*code|enrol/i.test(body)) {
  console.error(
    "Kiosk is still on the enrolment screen. The seeded session may have been " +
      "overwritten — re-run `make local-seed`. Page text:\n" +
      body.slice(0, 400),
  );
  await finish(browser, page, "/tmp/kiosk-scan-failed.png");
  process.exit(1);
}

console.log(`Kiosk ready at ${BASE_URL}/kiosk/${PROFILE}`);
console.log(
  `  session TestAKiosk02 at Test A Unit, key ${kioskKey.fingerprint}`,
);
console.log("  scan a member: 10000001 (Alice) or 10000002 (Bob)");
console.log("  from the other unit: 20000001 (Crossunit Tester)");
console.log(`\n${body.slice(0, 600)}`);

await finish(browser, page, "/tmp/kiosk-scan.png");
