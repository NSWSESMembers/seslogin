import {
  startAuthentication,
  startRegistration,
  browserSupportsWebAuthn,
  browserSupportsWebAuthnAutofill,
  platformAuthenticatorIsAvailable,
} from "@simplewebauthn/browser";
import { getGraphQLEndpoint } from "./api";
import { clientHeaders } from "./clientInfo";

export {
  browserSupportsWebAuthn,
  browserSupportsWebAuthnAutofill,
  platformAuthenticatorIsAvailable,
};

/**
 * Have the browser create a passkey for the registration options a begin
 * mutation returned (`optionsJson`): the `credentialJson` to finish with, or
 * `cancelled` if the person dismissed the prompt (or the device refused).
 */
export async function createPasskey(
  optionsJson: string,
): Promise<{ status: "ok"; credentialJson: string } | { status: "cancelled" }> {
  try {
    const response = await startRegistration({
      optionsJSON: JSON.parse(optionsJson),
    });
    return { status: "ok", credentialJson: JSON.stringify(response) };
  } catch (err) {
    console.warn("[passkey] startRegistration threw/aborted:", err);
    return { status: "cancelled" };
  }
}

const PASSKEY_LOGIN_FLAG = "passkey_login_session";

/** True if the current browser session was authenticated via passkey. */
export function wasPasskeyLoginSession(): boolean {
  return sessionStorage.getItem(PASSKEY_LOGIN_FLAG) === "1";
}

export function clearPasskeyLoginSession(): void {
  sessionStorage.removeItem(PASSKEY_LOGIN_FLAG);
}

const ENROLL_PROMPT_TS = "passkey_enroll_prompt_ts";
const ENROLL_PROMPT_INTERVAL_MS = 12 * 60 * 60 * 1000;

/** True if the enrollment interstitial was shown within the last 12 hours. */
export function passkeyEnrollPromptThrottled(): boolean {
  const raw = localStorage.getItem(ENROLL_PROMPT_TS);
  if (!raw) return false;
  const ts = Number.parseInt(raw, 10);
  if (Number.isNaN(ts)) return false;
  return Date.now() - ts < ENROLL_PROMPT_INTERVAL_MS;
}

/** Record that the enrollment interstitial was just shown. */
export function markPasskeyEnrollPromptShown(): void {
  localStorage.setItem(ENROLL_PROMPT_TS, String(Date.now()));
}

const ENROLL_FORCE_SHOW_EVENT = "passkey-enroll-force-show";

/**
 * Dev-only escape hatch: force the enrollment interstitial to appear regardless
 * of throttle, WebAuthn support, or whether a passkey already exists. Used by a
 * dev-only button on the admin home screen to preview the interstitial.
 */
export function forcePasskeyEnrollPrompt(): void {
  window.dispatchEvent(new Event(ENROLL_FORCE_SHOW_EVENT));
}

/** Subscribe to force-show requests; returns an unsubscribe function. */
export function onForcePasskeyEnrollPrompt(handler: () => void): () => void {
  window.addEventListener(ENROLL_FORCE_SHOW_EVENT, handler);
  return () => window.removeEventListener(ENROLL_FORCE_SHOW_EVENT, handler);
}

async function rawGraphQL(
  query: string,
  variables: Record<string, unknown>,
): Promise<{ data?: Record<string, unknown>; errors?: unknown[] }> {
  const resp = await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...clientHeaders(),
    },
    body: JSON.stringify({ query, variables }),
    cache: "no-store",
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const body = await resp.json();
  // Surface GraphQL errors — otherwise a failed finish/begin just looks like a
  // silent null and the login appears to "do nothing".
  if (body?.errors) {
    console.warn("[passkey] GraphQL returned errors:", body.errors);
  }
  return body;
}

/**
 * Result of a passkey login attempt.
 * - `ok`: authenticated, carries the session token.
 * - `cancelled`: the user dismissed/aborted the prompt (or autofill was
 *   superseded). Callers should stay silent — this isn't an error.
 * - `failed`: a real failure (challenge couldn't be issued, or the server
 *   rejected the assertion, e.g. the credential was deleted/unknown). Callers
 *   should surface a generic error.
 */
export type PasskeyLoginResult =
  | { status: "ok"; token: string }
  | { status: "cancelled" }
  | { status: "failed"; message?: string };

/**
 * The message of a `FORBIDDEN` GraphQL error, which the server words for people
 * (e.g. a Tally passkey used on the admin login). Other errors stay generic.
 */
function forbiddenMessage(errors: unknown[] | undefined): string | undefined {
  for (const e of errors ?? []) {
    const err = e as { message?: unknown; extensions?: { code?: unknown } };
    if (
      err.extensions?.code === "FORBIDDEN" &&
      typeof err.message === "string"
    ) {
      return err.message;
    }
  }
  return undefined;
}

/**
 * The first half of a discoverable (usernameless) passkey sign-in, shared by
 * the admin login and Tally sign-in: get a challenge from the server and have
 * the browser sign it with a passkey the person picks. Pass the result to the
 * flow's own finish mutation. With `useAutofill`, the browser offers saved
 * passkeys inline on the email field (conditional UI) instead of a modal.
 */
export type PasskeyAssertionResult =
  | { status: "ok"; challengeId: string; credentialJson: string }
  | { status: "cancelled" }
  | { status: "failed" };

export async function getPasskeyAssertion(opts?: {
  useAutofill?: boolean;
}): Promise<PasskeyAssertionResult> {
  const useAutofill = opts?.useAutofill ?? false;

  const beginResp = (await rawGraphQL(
    `mutation BeginPasskeyLogin {
      beginPasskeyLogin { challengeId optionsJson }
    }`,
    {},
  )) as {
    data?: {
      beginPasskeyLogin?: { challengeId: string; optionsJson: string } | null;
    };
  };

  const challenge = beginResp?.data?.beginPasskeyLogin;
  if (!challenge) {
    console.warn(
      "[passkey] beginPasskeyLogin returned no challenge",
      beginResp,
    );
    return { status: "failed" };
  }

  const optionsJSON = JSON.parse(challenge.optionsJson);

  let authResponse;
  try {
    authResponse = await startAuthentication({
      optionsJSON,
      useBrowserAutofill: useAutofill,
    });
  } catch (err) {
    // User cancelled, no matching credential, or an aborted autofill request.
    console.warn("[passkey] startAuthentication threw/aborted:", err);
    return { status: "cancelled" };
  }

  return {
    status: "ok",
    challengeId: challenge.challengeId,
    credentialJson: JSON.stringify(authResponse),
  };
}

/**
 * Attempt a discoverable (usernameless) passkey login to the admin site. With
 * `useAutofill`, the browser surfaces saved passkeys inline on the email field
 * (conditional UI) rather than showing a modal.
 */
export async function loginWithPasskey(opts?: {
  useAutofill?: boolean;
}): Promise<PasskeyLoginResult> {
  const assertion = await getPasskeyAssertion(opts);
  if (assertion.status !== "ok") return assertion;

  const finishResp = (await rawGraphQL(
    `mutation FinishPasskeyLogin($challengeId: String!, $credentialJson: String!) {
      finishPasskeyLogin(challengeId: $challengeId, credentialJson: $credentialJson)
    }`,
    {
      challengeId: assertion.challengeId,
      credentialJson: assertion.credentialJson,
    },
  )) as {
    data?: { finishPasskeyLogin?: string | null };
    errors?: unknown[];
  };

  const token = finishResp?.data?.finishPasskeyLogin ?? null;
  if (token) {
    sessionStorage.setItem(PASSKEY_LOGIN_FLAG, "1");
    return { status: "ok", token };
  }
  // The assertion was produced but the server didn't issue a token — e.g. the
  // credential was deleted server-side, or verification failed.
  console.warn(
    "[passkey] finishPasskeyLogin returned no token (verification failed or unknown credential)",
    finishResp,
  );
  return { status: "failed", message: forbiddenMessage(finishResp?.errors) };
}
