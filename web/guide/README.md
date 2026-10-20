# User guides — source

SES Login has two user guides, published on the site at **/guide** for anyone to
read, no sign-in needed:

| Folder   | Guide       | For                                        | On the site    |
| -------- | ----------- | ------------------------------------------ | -------------- |
| `admin/` | Admin guide | Unit administrators using the dashboard    | `/guide/admin` |
| `kiosk/` | Kiosk guide | Members signing in and out at a unit kiosk | `/guide/kiosk` |

Every `*.md` file in those folders (except any starting with `_`) is a page. The
web build compiles them into the app; readers can search both guides and print
either from `/guide/<guide>/print`. Screenshots for both live in `screenshots/`.

**Any change to what admins or kiosk users see or do should update the matching
guide in the same PR.** See [CONTRIBUTING.md](../../CONTRIBUTING.md#user-guide)
for the policy.

## Previewing

`npm run dev` (or `make dev` / `make dev-local`) and open
<http://localhost:5173/guide>. Saving a guide file reloads the page.

A guide mistake fails `npm run build` (and so CI) with the file and the
problem, for example:

```
admin/kiosks.md: link "members.md#adding-someone" — admin/members.md has no heading #adding-someone
```

## Writing a page

Each page starts with frontmatter:

```markdown
---
title: Setting up kiosks
summary: Turn a computer or tablet into a sign-in kiosk, and keep it running.
order: 30
video: 02-setting-up-a-kiosk
---

Intro paragraph — shown before the first heading.

## A section

...
```

| Field     | Required | Meaning                                                                                                                                               |
| --------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `title`   | yes      | Page heading and its name in the contents and search results.                                                                                         |
| `summary` | yes      | One sentence under the title and on the contents page.                                                                                                |
| `order`   | yes      | Position in the contents. Leave gaps (10, 20, 30…) so pages can go in between.                                                                        |
| `video`   | no       | Name of the walkthrough video for this page. Recorded now; **not shown yet** — video hosting is still to be decided, so the guide renders without it. |

The file and folder make the page's address: `admin/kiosks.md` is
`/guide/admin/kiosks`. Use lower-case-with-dashes, and don't rename a page
lightly — people bookmark and share these links. Each guide starts with a
`quick-start.md`.

### What you can use

- `##` and `###` headings. Not `#` — the title comes from frontmatter. Every
  `##` appears in the page's "On this page" list, and every `##`/`###` section is
  its own search result, so give sections names people would search for.
- Ordinary Markdown: paragraphs, **bold**, lists, tables, `code`.
- Links:
  - another page in the same guide: `[Kiosks](kiosks.md)`, or a section of
    one: `[Kiosk settings](kiosks.md#kiosk-settings)`
  - a page in the other guide: `[Signing out](../kiosk/signing-out.md)`
  - a section of the same page: `[below](#reactivating-a-kiosk)`
  - a screen of the app: `[Members](/admin/members)`
  - outside: `https://…` (opens in a new tab)

  Links between guide pages are **checked at build time**. A heading's anchor is
  its text in lower case with spaces turned into dashes and punctuation
  dropped: `## Who's here` becomes `#whos-here`.

- Screenshots, on a line of their own:

  ```markdown
  ![The Kiosks page, with the Reactivate button highlighted](../screenshots/kiosks-list.png "Bring an expired kiosk back.")
  ```

  The alt text (in `[...]`) is required: describe what the picture shows for
  someone who can't see it. The quoted title becomes the caption. See
  [Screenshots](#screenshots) below for making them.

- Callouts:

  ```markdown
  > [!TIP]
  > Bookmark the kiosk page so it's easy to get back to.
  ```

  `NOTE`, `TIP`, `IMPORTANT` and `WARNING` are available.

Not supported, and rejected by the build: raw HTML, and images anywhere but
`../screenshots/`.

## Screenshots

Screenshots are **generated, not taken by hand**, so they all look the same and
can be refreshed whenever the UI changes. `local/guide/shots.mjs` drives the app on
the local stack — the dashboard for the admin guide, a real kiosk for the kiosk
guide — rings the elements a page talks about in orange (with an optional label),
and crops to the part of the screen that matters. Everything on screen is the
invented demo unit from `local/guide/seed-demo.mjs`.

It uses Playwright, which isn't a dependency of the repo — install it at the repo
root once, as for [local/examples](../../local/examples/README.md):
`npm i playwright --no-save && npx playwright install chromium`.

```bash
make local-e2e                         # the local stack (no AWS needed)
make guide-screenshots                 # re-capture all of them
make guide-screenshots SHOTS=kiosk-    # just the ones named kiosk-*
```

To add one, add a `snap(...)` to a scene in `local/guide/shots.mjs` (the file
explains how), capture it, and reference it from the page. **When a UI change
alters a screen that's pictured, re-capture it in the same PR** — a stale
screenshot is as misleading as stale text. The build fails if a page references a
screenshot that doesn't exist (the dev server shows a placeholder instead, so you
can write the reference first).

Commit the PNGs. They are captured at 2x and shown at half size, so they stay
sharp on high-resolution screens.

## Style

- Write for a unit volunteer, not a developer. Say what to press and what
  happens, in the words the screen uses (bold the button or menu name:
  **Kiosks**, **Save**).
- The kiosk guide is for members, who may never see the dashboard: keep it short,
  and send anything an administrator has to do to the admin guide.
- The product is **SES Login**. NITCs live in **Beacon**.
- Australian spelling (organise, colour).
- In the admin guide, say when something needs the **Admin** role rather than
  **Read only** — see `admin/roles-and-access.md`.
- Keep each quick start short; detail belongs in the topic pages.
- The guides are public: never put anything in them that only signed-in users
  should see.
