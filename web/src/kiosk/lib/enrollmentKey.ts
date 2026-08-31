/**
 * Publishing a kiosk's public key for enrollment, shared by the enrollment screen and
 * the re-enroll section of the kiosk status dialog.
 */

import { getGraphQLEndpoint } from "../../lib/api";
import {
  CLIENT_VERSION_HEADER,
  getCurrentClientVersion,
} from "../../lib/clientVersion";
import { buildSignedAuthHeader, type KioskKeyInfo } from "./kioskKey";

// Re-publish the public key this often while the kiosk shows its QR code. The server
// keeps the pending record for 30 min, so 10 min keeps it comfortably alive.
export const ENROLL_SUBMIT_INTERVAL_MS = 10 * 60 * 1000;

export function enrollHeaders(): Record<string, string> {
  return {
    "Content-Type": "application/json",
    [CLIENT_VERSION_HEADER]: getCurrentClientVersion(),
  };
}

/** The admin page a scanned enrollment QR code opens, pre-filled for this device. */
export function enrollUrlForFingerprint(fingerprint: string): string {
  return `${window.location.origin}/admin/sessions/enroll?fp=${fingerprint}`;
}

/**
 * Publish the public key as a pending enrollment (unauthenticated). The admin enroll
 * page refuses to enroll a fingerprint with no live pending record, so this has to
 * succeed before the QR code is worth scanning — hence throwing rather than ignoring
 * a failed response.
 */
export async function submitEnrollmentKey(info: KioskKeyInfo): Promise<void> {
  const body = JSON.stringify({
    query:
      "mutation KioskSubmitEnrollmentKey($publicKey: String!) { submitEnrollmentKey(publicKey: $publicKey) }",
    variables: { publicKey: info.publicKeySpkiB64 },
  });
  const resp = await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers: enrollHeaders(),
    body,
    cache: "no-store",
  });
  if (!resp.ok) {
    throw new Error(`Server returned ${resp.status}`);
  }
  const json = await resp.json();
  if (json?.errors?.length > 0) {
    throw new Error(json.errors[0]?.message ?? "Unknown error");
  }
}

/**
 * Ask the server which session this device's key is bound to, by making a signed
 * request. Resolves the session id once the key is enrolled, or null while it is not
 * (a 401 is expected until an admin enrolls it).
 */
export async function fetchKeySessionId(
  info: KioskKeyInfo,
): Promise<string | null> {
  const body = JSON.stringify({
    query: "query KioskEnrollPoll { session { id } }",
  });
  const authorization = await buildSignedAuthHeader(info, body);
  const resp = await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers: { ...enrollHeaders(), Authorization: authorization },
    body,
    cache: "no-store",
  });
  if (!resp.ok) {
    return null;
  }
  const json = await resp.json();
  const id = json?.data?.session?.id;
  return typeof id === "string" ? id : null;
}
