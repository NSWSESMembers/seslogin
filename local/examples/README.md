# Playwright examples for the local stack

Two scripts that put a browser into a state worth testing from, so you don't spend the
first ten minutes of every session logging in by hand.

| Script                               | Leaves you at                                                                                        |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| [`kiosk-scan.mjs`](kiosk-scan.mjs)   | The kiosk scan screen, signed in as the seeded key-enrolled kiosk — ready to sign a member in or out |
| [`admin-login.mjs`](admin-login.mjs) | The admin dashboard, as a chosen user with a chosen location selected                                |

Both assume the local stack is up and seeded:

```bash
make local-e2e
```

Playwright isn't a dependency of this repo — install it wherever you like and point Node
at it:

```bash
npm i playwright && npx playwright install chromium
node local/examples/admin-login.mjs
```

If the machine already has a Chromium (CI images and sandboxes usually do), set
`CHROMIUM_PATH` instead of downloading one:

```bash
CHROMIUM_PATH=/opt/pw-browsers/chromium-1194/chrome-linux/chrome node local/examples/kiosk-scan.mjs
```

Both scripts run headless by default, print what they did, and take a screenshot. Set
`HEADED=1` to watch, and `KEEP_OPEN=1` to leave the browser open so you can carry on by
hand — which is usually the point.

```bash
HEADED=1 KEEP_OPEN=1 node local/examples/kiosk-scan.mjs
```

Options are environment variables so the scripts stay copy-paste-able; see the header of
each. Shared setup lives in [`fixtures.mjs`](fixtures.mjs), which reads the same seed
files the database is loaded from, so the two can't drift.
