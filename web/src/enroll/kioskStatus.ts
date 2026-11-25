import { formatFullDateTime } from "../lib/time";

export type KioskStatus =
  | { kind: "this" }
  | { kind: "none" }
  | { kind: "online" }
  | { kind: "offline"; since: number | null };

/** Seen this recently, a kiosk is probably still running on its computer (the kiosk list's "online" band). */
export const RECENT_CONTACT_SECS = 20 * 60;

/** Where a kiosk stands, from the computer being set up's point of view. */
export function kioskStatus(
  session: {
    readonly id: string;
    readonly keyEnrolled: boolean;
    readonly keyReleasedAt?: number | null;
    readonly lastContact?: number | null;
  },
  currentId: string | null,
  now: number,
): KioskStatus {
  if (session.id === currentId) return { kind: "this" };
  if (!session.keyEnrolled || session.keyReleasedAt != null)
    return { kind: "none" };
  if (
    session.lastContact != null &&
    now - session.lastContact < RECENT_CONTACT_SECS
  )
    return { kind: "online" };
  return { kind: "offline", since: session.lastContact ?? null };
}

export function describeStatus(status: KioskStatus): string {
  switch (status.kind) {
    case "this":
      return "this computer";
    case "none":
      return "no computer";
    case "online":
      return "online on another computer";
    case "offline":
      return status.since == null
        ? "never seen"
        : `offline since ${formatFullDateTime(new Date(status.since * 1000))}`;
  }
}
