# Announcement emails

HTML emails announcing new SES Activity features to administrators, sent through the
Toolbox broadcast email feature. Nothing in the codebase reads this directory. It is a
record of what was sent, plus a starting point for the next email.

## Layout

```
email/
├── README.md
├── template/                  # copy this to start a new email
│   ├── index.html
│   └── images/
│       └── logo.png
└── 2026-11-03-short-slug/     # one directory per email, named for its send date
    ├── index.html
    └── images/
```

Name each email's directory `YYYY-MM-DD-short-slug`, using the date it is to be sent, so
the directories sort in send order.

## Writing a new email

1. Copy the template: `cp -r email/template email/2026-11-03-short-slug`.
2. Replace every `{{...}}` placeholder in `index.html`:
   - `<title>`: the email's subject line.
   - Preview text: the line inbox lists show after the subject. It is hidden in the
     email itself.
   - Summary: the small grey line above the header.
   - Title suffix: the orange line under "SES Activity", e.g. "New features".
   - Main body: everything in the white cell between the header and the footer. Delete
     the screenshot, list or button if the email doesn't need them.
3. Leave the footer alone unless the wording itself needs to change, in which case change
   `template/` too.
4. Put the email's images in its own `images/` directory. Export screenshots at twice the
   size they're shown at (1104px wide for a full-width image) and set the display size
   with the `width` attribute.
5. Before sending, make sure image `src`s will work in the recipient's inbox. Email
   clients can't resolve the relative `images/...` paths used here. Either let the
   broadcast tool embed the images, or upload them somewhere public and use absolute
   URLs.
6. Send a test to yourself and check it in Gmail (web and mobile) and Outlook (desktop on
   Windows if possible) before broadcasting.

## Rules for the HTML

Gmail and Outlook support far less HTML and CSS than a browser does. Outlook on Windows
renders with Word. To keep emails readable in both:

- Lay out with `<table role="presentation">`, not `div`s, flexbox or grid.
- Keep the content 600px wide.
- Put every style inline (`style="..."`). The `<style>` block in the template is only
  progressive enhancement (mobile padding, fluid images). Clients that drop it must still
  show a working email.
- Use PNG or JPG images with `width`, `height` (or `height: auto`) and `alt` set. SVG
  doesn't work in Gmail or Outlook.
- Use web-safe fonts. The template uses Arial.
- Keep the `<!--[if mso]>` blocks. They are what Outlook on Windows draws: a fixed-width
  wrapper and the button.
- Use `<p style="margin: ...">` for spacing. Outlook ignores margins on most other
  elements.
