/**
 * The admin user guide's screenshots. Each scene drives the app into a state and
 * calls `snap(page, name, { highlight, frame })`, which saves
 * web/guide/screenshots/<name>.png, cropped around `frame` (or the highlights),
 * with an orange ring — and optional label — around each highlighted element.
 *
 * Adding a screenshot for a guide page: add a snap here (to an existing scene, or
 * a new one at the end), run `node local/guide/capture.mjs <name>`, and reference
 * it from the page as `![What it shows](screenshots/<name>.png)`.
 *
 * Everything on screen comes from local/guide/seed-demo.mjs: invented people only.
 */

const row = (page, text) => page.locator("tr", { hasText: text }).first();
const cell = (page, text, i) => row(page, text).locator("td").nth(i);
const lastCell = (page, text) => row(page, text).locator("td").last();

export const SHOTS = [
  {
    name: "signing-in",
    async run({ open, snap, settle }) {
      const page = await open("/admin", { signedIn: false });
      const email = page.locator('input[type="email"]');
      await email.fill("testunit@seslogin.test");
      await snap(page, "sign-in-email", {
        highlight: [
          { locator: email, label: "Your email address", where: "right" },
          {
            locator: page.getByRole("button", { name: "Send code" }),
            where: "below",
          },
        ],
        frame: [
          page.getByText("Please sign in to continue"),
          page.getByRole("button", { name: "Send code" }),
        ],
      });

      await page.getByRole("button", { name: "Send code" }).click();
      await settle(page);
      const code = page.getByPlaceholder("6-digit code");
      await snap(page, "sign-in-code", {
        highlight: [
          { locator: code, label: "The code from your email", where: "right" },
          { locator: page.getByRole("button", { name: "Verify code" }) },
        ],
        frame: [
          page.getByText("Please sign in to continue"),
          page.getByRole("button", { name: "Start over" }),
        ],
      });

      const prompt = await open("/admin", { passkeyPrompt: true });
      await snap(prompt, "passkey-prompt", {
        highlight: [
          {
            locator: prompt.getByRole("button", { name: "Add a passkey" }),
            label: "Recommended",
            where: "right",
          },
          { locator: prompt.getByText("Maybe later"), where: "right" },
        ],
        frame: [
          prompt.getByRole("heading", { name: "Add a passkey" }),
          prompt.getByText("Maybe later"),
        ],
      });
    },
  },

  {
    name: "dashboard",
    async run({ open, snap }) {
      const page = await open("/admin");
      const menu = page.locator("#admin-menu-links");
      await snap(page, "menu-bar", {
        highlight: [
          { locator: menu.getByRole("link", { name: "User guide" }) },
        ],
        frame: [page.getByText("Test A Unit").first(), menu],
        pad: 6,
      });
      await snap(page, "dashboard", {
        frame: [
          page.getByText("Quick insights"),
          page.getByText(/^Avg completed period/),
        ],
      });
    },
  },

  {
    name: "kiosks",
    async run({ open, snap, settle, APP, PHONE_APP }) {
      // Setting up a kiosk: the kiosk shows a QR code, the admin's phone opens it.
      const kiosk = await open("/kiosk", { signedIn: false, width: 1280 });
      const qr = kiosk.getByAltText("Kiosk enrollment QR code");
      await qr.waitFor();
      await snap(kiosk, "kiosk-qr", {
        highlight: [
          { locator: qr, label: "Scan this with your phone", where: "right" },
        ],
        frame: [kiosk.getByRole("heading", { name: "Enroll this kiosk" }), qr],
      });

      const href = await kiosk
        .locator('a[href*="/enroll?fp="]')
        .first()
        .getAttribute("href");
      const phone = await open(href.replace(APP, PHONE_APP), { phone: true });
      await phone.getByText("Choose the location").waitFor();
      await phone.locator("#locationId").selectOption("TestAUnit001");
      await phone.locator("#name").fill("Front desk");
      await snap(phone, "kiosk-enrol", {
        highlight: [
          {
            locator: phone.locator("#locationId"),
            label: "Your unit",
            where: "above",
          },
          {
            locator: phone.locator("#name"),
            label: "Where the kiosk is",
            where: "above",
          },
        ],
        frame: [
          phone.getByRole("heading", { name: "Enroll this kiosk" }),
          phone.locator("#name"),
        ],
        pad: 16,
        minWidth: 0,
      });

      const mode = phone.getByRole("radiogroup", { name: "Mode" });
      // Forms are a <dl>: the "Options" label is a <dt>, its checkboxes the next <dd>.
      const options = phone
        .getByText("Options", { exact: true })
        .locator("xpath=ancestor::dt[1]/following-sibling::dd[1]");
      await snap(phone, "kiosk-settings", {
        highlight: [
          { locator: mode, label: "Mode", where: "above" },
          { locator: options, label: "Options", where: "above" },
        ],
        frame: [mode, options],
        pad: 16,
        minWidth: 0,
      });

      await phone.getByRole("button", { name: "Save" }).click();
      await settle(phone);
      const ready = kiosk.getByText("Please enter or scan your SES ID");
      await ready.waitFor({ timeout: 30_000 });
      await settle(kiosk);
      const input = kiosk.locator("input").first();
      await snap(kiosk, "kiosk-ready", {
        highlight: [
          {
            locator: input,
            label: "Members enter their SES ID here",
            where: "below",
          },
        ],
        frame: [kiosk.getByText(/Front desk/).first(), ready, input],
        pad: 0,
      });

      // The Kiosks page, with the kiosk just set up on it.
      const page = await open("/admin/sessions");
      await snap(page, "kiosks-list", {
        highlight: [
          {
            locator: [cell(page, "Front desk", 0), cell(page, "Front desk", 1)],
            label: "Health light",
            where: "below",
          },
          {
            locator: cell(page, "Front desk", 2),
            label: "Last contact",
            where: "above",
          },
          {
            locator: lastCell(page, "Front desk"),
            label: "Change or remove",
            where: "above",
          },
        ],
        frame: [page.getByText("Expired/Unused"), page.locator("table")],
      });
    },
  },

  {
    name: "members",
    async run({ open, snap, settle }) {
      const page = await open("/admin/members");
      const filter = page.getByPlaceholder(/Filter by name/);
      await snap(page, "members-list", {
        highlight: [
          { locator: filter, label: "Find someone", where: "right" },
          {
            locator: [cell(page, "Bob Brown", 0), cell(page, "Bob Brown", 2)],
            label: "Green dot: kept up to date from SES",
            where: "right",
          },
          { locator: lastCell(page, "Alice Anderson") },
        ],
        frame: [filter, row(page, "Grace MacLeod")],
      });

      await page.getByRole("link", { name: "New", exact: true }).click();
      await settle(page);
      await page.locator("#givenname").fill("Jamie");
      await page.locator("#surname").fill("Collins");
      await page.locator("#serialnumber").fill("10000015");
      await snap(page, "members-new", {
        highlight: [
          {
            locator: page.getByRole("button", { name: "Save" }),
            where: "right",
          },
        ],
        frame: [
          page.getByText("Enter the details of the new member"),
          page.getByRole("button", { name: "Save" }),
        ],
      });
      // Really add them, so the audit log has an admin's change to show.
      await page.getByRole("button", { name: "Save" }).click();
      await settle(page);
    },
  },

  {
    name: "activity",
    async run({ open, snap, settle }) {
      const page = await open("/admin/activity");
      const filter = page.getByPlaceholder(/Filter by name/);
      const firstDay = page
        .getByText(
          /^(Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday)\b/,
        )
        .first();
      await snap(page, "activity-list", {
        highlight: [
          { locator: [filter, page.getByText(/^Categories:/).first()] },
          { locator: firstDay, label: "Grouped by day", where: "right" },
          {
            locator: page
              .getByText("Edit", { exact: true })
              .first()
              .locator("xpath=ancestor::td[1]"),
            label: "Edit or delete",
            where: "left",
          },
        ],
        frame: [filter, page.locator("tbody tr").nth(4)],
      });

      await page.getByRole("link", { name: "Incomplete Periods" }).click();
      await settle(page);
      await snap(page, "incomplete-periods", {
        highlight: [
          {
            locator: [cell(page, "Bob Brown", 0), cell(page, "Bob Brown", 2)],
            label: "Signed in yesterday, never signed out",
            where: "below",
          },
          {
            locator: lastCell(page, "Bob Brown"),
            label: "Fix it",
            where: "above",
          },
        ],
        frame: [
          page.getByText("This list shows members currently signed in"),
          page.locator("table"),
        ],
      });

      await lastCell(page, "Bob Brown")
        .getByText("Edit", { exact: true })
        .click();
      await settle(page);
      const end = page.locator("#end");
      await snap(page, "activity-edit", {
        highlight: [
          {
            locator: end.locator(".."),
            label: "Set the right finish time",
            where: "right",
          },
          {
            locator: page.getByRole("button", { name: "Save" }),
            where: "right",
          },
        ],
        frame: [
          page.getByText("Edit the activity entry details"),
          page.getByRole("button", { name: "Save" }),
        ],
      });
    },
  },

  {
    name: "reports",
    async run({ open, snap }) {
      const page = await open("/admin/activity/totals");
      await snap(page, "totals", {
        highlight: [
          {
            locator: page.getByText("Start time:").locator(".."),
            label: "Choose your dates",
            where: "above",
          },
          {
            locator: page.getByRole("button", { name: "Update results" }),
            label: "Then update",
            where: "right",
          },
        ],
        // The totals aren't a <table>; frame down to the fifth member's hours.
        frame: [
          page.getByText("Totals include everyone"),
          page.getByText(/^\d+ hours?$/).nth(4),
        ],
      });

      const heat = await open("/admin/activity/heatmap");
      const ranges = [
        heat.getByRole("button", { name: "Last 30 days" }),
        heat.getByRole("button", { name: /^Max/ }),
      ];
      await snap(heat, "heatmap", {
        highlight: [
          { locator: ranges, label: "Choose a range", where: "right" },
        ],
        frame: [
          heat.getByText("This heatmap covers"),
          heat.getByText("Grace MacLeod").last(),
          ...ranges,
        ],
      });

      const rep = await open("/admin/reports");
      const downloads = [
        rep.getByRole("button", { name: "Download CSV" }),
        rep.getByRole("button", { name: "Download XLSX" }),
      ];
      await snap(rep, "reports-download", {
        highlight: [
          { locator: downloads, label: "Excel or CSV", where: "right" },
        ],
        frame: [
          rep.getByText("This report includes every sign-in"),
          ...downloads,
        ],
      });
    },
  },

  {
    name: "audit-log",
    async run({ open, snap }) {
      const page = await open("/admin/audit");
      await snap(page, "audit-log", {
        highlight: [
          {
            locator: page.locator("#auditEntityTypes").locator(".."),
            label: "Show only some kinds of change",
            where: "right",
          },
          { locator: page.getByRole("button", { name: "Details" }).first() },
        ],
        frame: [
          page.getByText(/^Changes made at/),
          page.locator("tbody tr").nth(1),
        ],
      });
    },
  },

  {
    name: "settings",
    async run({ open, snap }) {
      const page = await open("/admin/settings/daily-email");
      const unit = page.getByLabel("Test A Unit");
      await unit.check();
      await snap(page, "daily-email", {
        highlight: [
          {
            locator: [
              unit,
              page.getByText("Test A Unit", { exact: true }).last(),
            ],
            label: "Tick your unit",
            where: "right",
          },
          {
            locator: page.getByRole("button", { name: "Save" }),
            where: "right",
          },
        ],
        frame: [
          page.getByText("Choose which locations"),
          page.getByRole("button", { name: "Save" }),
        ],
      });
    },
  },

  {
    name: "kiosk-guide",
    async run({ open, snap, settle, PHONE_APP, APP }) {
      // A second kiosk with every member-facing option on, for the kiosk guide.
      const kiosk = await open("/kiosk", { signedIn: false, width: 1280 });
      const href = await kiosk
        .locator('a[href*="/enroll?fp="]')
        .first()
        .getAttribute("href");
      const phone = await open(href.replace(APP, PHONE_APP), { phone: true });
      await phone.getByText("Choose the location").waitFor();
      await phone.locator("#locationId").selectOption("TestAUnit001");
      await phone.locator("#name").fill("Training room");
      for (const option of await phone.locator('input[type="checkbox"]').all())
        await option.check();
      await phone.getByRole("button", { name: "Visible + Sign out" }).click();
      await phone.getByRole("button", { name: "Save" }).click();
      await kiosk
        .getByText("Please enter or scan your SES ID")
        .first()
        .waitFor({ timeout: 30_000 });
      await settle(kiosk);

      // Every kiosk screen stays mounted off to the side; only one is centred.
      const screen = () => kiosk.locator("div.translate-x-0").first();
      const enter = async (id) => {
        const box = screen().locator("input").first();
        await box.fill(id);
        await box.press("Enter");
        await kiosk.waitForTimeout(1200);
      };

      await enter("10000003");
      const message = screen().getByText(/signed in at/);
      await snap(kiosk, "kiosk-signed-in", {
        highlight: [{ locator: message, where: "below" }],
        frame: [
          screen().getByText("Please enter or scan your SES ID"),
          message,
        ],
      });

      const list = kiosk.getByText("Currently signed in").first().locator("..");
      await snap(kiosk, "kiosk-whos-here", {
        highlight: [
          { locator: list.getByRole("button", { name: "Sign out" }).first() },
        ],
        frame: [
          kiosk.getByText("Currently signed in").first(),
          list.getByRole("button", { name: "Sign out" }).last(),
        ],
        minWidth: 0,
      });

      // Crossunit Tester belongs to Test B Unit, so Confirm visitors asks first.
      // No leaves nothing behind for the shots after this one.
      await enter("20000001");
      const question = kiosk.getByRole("heading", { name: /^Are you / });
      const yes = kiosk.getByRole("button", { name: "Yes, sign me in" });
      await snap(kiosk, "kiosk-confirm-visitor", {
        highlight: [{ locator: yes, where: "below" }],
        frame: [question, yes],
        minWidth: 0,
      });
      await kiosk.getByRole("button", { name: "No", exact: true }).click();

      await kiosk.waitForTimeout(3000);
      await enter("10000001");
      await snap(kiosk, "kiosk-quick-pick", {
        highlight: [
          {
            locator: screen().getByRole("button", { name: /More categories/ }),
            where: "right",
          },
        ],
        // Every option, so the outer cards aren't cut off.
        frame: [
          screen().getByText("Quick pick"),
          ...(await screen().getByRole("button").all()),
        ],
      });

      await screen()
        .getByRole("button", { name: /More categories/ })
        .click();
      await kiosk.waitForTimeout(900);
      await snap(kiosk, "kiosk-categories", {
        // Every category group, so none is cut off at the edge.
        frame: [
          screen().getByText("Categories", { exact: true }),
          ...(await screen().getByRole("button").all()),
        ],
      });

      // Category search is one of the options ticked above.
      const search = screen().getByRole("searchbox", {
        name: "Search categories",
      });
      await search.fill("stor");
      await kiosk.waitForTimeout(300);
      await snap(kiosk, "kiosk-category-search", {
        highlight: [{ locator: search, where: "right" }],
        frame: [
          screen().getByText("Categories", { exact: true }),
          search,
          ...(await screen().getByRole("button").all()).slice(0, 6),
        ],
      });
      await screen().getByRole("button", { name: "Clear search" }).click();

      await screen()
        .getByRole("button", { name: "Training", exact: true })
        .click();
      await kiosk.waitForTimeout(900);
      await screen()
        .getByRole("button", { name: "First Aid", exact: true })
        .click();
      await kiosk.waitForTimeout(1200);
      const edits = screen().getByRole("button", { name: "Edit" });
      await snap(kiosk, "kiosk-confirm", {
        highlight: [
          {
            locator: [edits.first(), edits.last()],
            label: "Fix a time",
            where: "right",
          },
          {
            locator: screen().getByRole("button", { name: "Submit" }),
            where: "right",
          },
        ],
        frame: [
          screen().getByText("Confirm", { exact: true }),
          screen().getByText("Start time:"),
          screen().getByRole("button", { name: "Submit" }),
        ],
      });
      await screen().getByRole("button", { name: "Submit" }).click();
      await kiosk.waitForTimeout(3500);

      await enter("10000002");
      const answers = [
        screen().getByRole("button", { name: "Yeah, I forgot" }),
        screen().getByRole("button", { name: "Nope, this is a long entry" }),
      ];
      await snap(kiosk, "kiosk-forgot", {
        highlight: [{ locator: answers }],
        frame: [
          screen().getByText("Did you forget to sign out?"),
          screen().getByText(/That's about/),
          ...answers,
        ],
      });
      await kiosk.getByRole("button", { name: "Cancel sign out" }).click();
      await kiosk.waitForTimeout(1500);

      await screen()
        .getByRole("button", { name: "Guest sign in / out" })
        .click();
      // The guest dialog has no dialog role: find it by its title instead.
      const dialog = kiosk
        .getByText("Guest sign in / out", { exact: true })
        .last()
        .locator("xpath=ancestor::div[.//input][1]");
      await dialog.getByText("Name", { exact: true }).waitFor();
      const inputs = dialog.locator("input");
      await inputs.nth(0).fill("Pat Visitor");
      await inputs.nth(1).fill("Tour of the unit");
      await snap(kiosk, "kiosk-guest", {
        highlight: [
          {
            locator: dialog.getByRole("button", { name: "Sign in" }),
            where: "right",
          },
        ],
        frame: dialog,
      });
    },
  },

  {
    name: "guide",
    async run({ open, snap }) {
      // The guide itself, for the admin quick start's "Using this guide".
      const home = await open("/", { signedIn: false, width: 1280 });
      const guideButton = home.getByRole("link", { name: /^Guide/ });
      await snap(home, "home-guide-button", {
        highlight: [{ locator: guideButton }],
        frame: [home.getByRole("link", { name: /^Kiosk/ }), guideButton],
      });

      const chooser = await open("/guide", { signedIn: false, width: 1280 });
      await snap(chooser, "guide-home", {
        frame: [
          chooser.getByRole("heading", { level: 1 }),
          chooser.getByRole("main").getByRole("listitem").last(),
        ],
      });

      const contents = await open("/guide/admin?q=kiosk%20expired", {
        signedIn: false,
        width: 1280,
      });
      const results = contents.getByRole("region", { name: "Search results" });
      await snap(contents, "guide-search", {
        highlight: [{ locator: contents.locator("#guide-search") }],
        frame: [
          contents.getByRole("heading", { level: 1 }),
          results.getByRole("listitem").nth(1),
        ],
      });

      const page = await open("/guide/admin/kiosks", {
        signedIn: false,
        width: 1280,
      });
      const topics = page
        .getByRole("navigation", { name: "Admin guide topics" })
        .first();
      const onThisPage = page
        .getByRole("navigation", { name: "On this page" })
        .last();
      await snap(page, "guide-page", {
        highlight: [
          { locator: topics, label: "Topics", where: "below" },
          { locator: onThisPage, label: "Sections", where: "below" },
          {
            locator: page.getByRole("button", { name: "Print this page" }),
            where: "right",
          },
        ],
        frame: [
          page.getByRole("navigation", { name: "Breadcrumb" }),
          topics,
          onThisPage,
        ],
      });
    },
  },
];
