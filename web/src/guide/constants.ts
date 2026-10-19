/** Where the guides are mounted (see GuideApp.tsx). */
export const GUIDE_BASE = "/guide";

/**
 * The guides, in the order they're offered. Each is a folder of Markdown pages
 * in web/guide/<id>/ and lives at /guide/<id>.
 */
export const GUIDES = [
  {
    id: "admin",
    title: "Admin guide",
    audience: "For unit administrators",
    description:
      "Set up kiosks, look after members, fix activity and run reports for your unit.",
  },
  {
    id: "kiosk",
    title: "Kiosk guide",
    audience: "For members",
    description:
      "Sign in and out at your unit's kiosk, pick what you did, and fix your times.",
  },
] as const;

export type GuideId = (typeof GUIDES)[number]["id"];

export const GUIDE_IDS: readonly string[] = GUIDES.map((g) => g.id);

/** Guide routes that are not pages, so no guide page may take these names. */
export const RESERVED_SLUGS = ["print"];
