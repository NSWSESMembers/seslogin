import { Turnstile, type TurnstileInstance } from "@marsidev/react-turnstile";
import { useEffect, useRef, useState } from "react";
import { useLocation } from "react-router";

import { getGraphQLEndpoint } from "../lib/api";
import { clientHeaders } from "../lib/clientInfo";
import {
  browserSupportsWebAuthn,
  browserSupportsWebAuthnAutofill,
  createPasskey,
  getPasskeyAssertion,
  platformAuthenticatorIsAvailable,
} from "../lib/passkey";
import LoadingIndicator from "../components/LoadingIndicator";
import {
  Panel,
  PanelBox,
  PanelIntro,
  PanelMessage,
  PanelTitle,
} from "../components/ui/Panel";
import { Button } from "../components/ui/Button";
import { Muted } from "../components/ui/Muted";
import TextInput from "../components/ui/TextInput";

const TURNSTILE_SITE_KEY =
  import.meta.env.VITE_TURNSTILE_DISABLED === "1"
    ? ""
    : (import.meta.env.VITE_TURNSTILE_SITE_KEY ?? "");

/** The OAuth authorization request the app opened this page with. */
interface AuthorizationRequest {
  clientId: string;
  redirectUri: string;
  codeChallenge: string;
  codeChallengeMethod: string;
  scope: string | null;
  resource: string | null;
  state: string | null;
}

function readRequest(search: string): AuthorizationRequest | null {
  const params = new URLSearchParams(search);
  const clientId = params.get("client_id");
  const redirectUri = params.get("redirect_uri");
  const codeChallenge = params.get("code_challenge");
  if (!clientId || !redirectUri || !codeChallenge) return null;
  if (params.get("response_type") && params.get("response_type") !== "code") {
    return null;
  }
  return {
    clientId,
    redirectUri,
    codeChallenge,
    codeChallengeMethod: params.get("code_challenge_method") ?? "S256",
    scope: params.get("scope"),
    resource: params.get("resource"),
    state: params.get("state"),
  };
}

interface GraphQLReply<T> {
  data?: T | null;
  errors?: { message: string; extensions?: { code?: string } }[];
}

/**
 * A refusal the server words for people (a passkey it doesn't know, or whose
 * account has no member), or null for anything else.
 */
function refusalMessage(reply: GraphQLReply<unknown>): string | null {
  const err = reply.errors?.[0];
  const code = err?.extensions?.code;
  return err && (code === "BAD_REQUEST" || code === "FORBIDDEN")
    ? err.message
    : null;
}

async function graphql<T>(
  query: string,
  variables: Record<string, unknown>,
): Promise<GraphQLReply<T>> {
  const resp = await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers: { "Content-Type": "application/json", ...clientHeaders() },
    body: JSON.stringify({ query, variables }),
    cache: "no-store",
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  return (await resp.json()) as GraphQLReply<T>;
}

function LinkProblem() {
  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Can't sign in here</PanelTitle>
        <PanelIntro>
          This sign-in link isn't one SESLogin recognises. Open the Tally app
          and sign in from there.
        </PanelIntro>
      </PanelBox>
    </Panel>
  );
}

type Step =
  | "email"
  | "sending"
  | "code"
  | "verifying"
  | "passkey"
  | "offer"
  | "saving"
  | "done";

/** How long "Not now" holds off the offer to save a passkey, per email. */
const OFFER_DISMISSED_FOR_MS = 30 * 24 * 60 * 60 * 1000;

function offerDismissedKey(email: string): string {
  return `tally_passkey_offer_dismissed:${email.trim().toLowerCase()}`;
}

/** Whether "Not now" was chosen for this email on this device lately. */
function offerDismissed(email: string): boolean {
  try {
    const at = Number(localStorage.getItem(offerDismissedKey(email)));
    return at > 0 && Date.now() - at < OFFER_DISMISSED_FOR_MS;
  } catch {
    return false;
  }
}

function dismissOffer(email: string) {
  try {
    localStorage.setItem(offerDismissedKey(email), String(Date.now()));
  } catch {
    // Storage blocked: they'll be asked again next time.
  }
}

/** Whether this device can save a passkey for the offer to be worth making. */
async function canSavePasskey(email: string): Promise<boolean> {
  if (!browserSupportsWebAuthn() || offerDismissed(email)) return false;
  try {
    return await platformAuthenticatorIsAvailable();
  } catch {
    return false;
  }
}

/**
 * Tally's sign-in page, at `/tally/sign-in`: the OAuth authorization endpoint for
 * member sign-in clients. Members (and users with a member) enter their email,
 * get a code, and are sent back to the app with an authorization code. It never
 * signs anyone in to SESLogin itself.
 */
export default function TallySignIn() {
  const { search } = useLocation();
  const [request] = useState(() => readRequest(search));
  const [clientName, setClientName] = useState<string | null>(null);
  const [isDevBuild, setIsDevBuild] = useState(false);
  const [linkOk, setLinkOk] = useState<boolean | null>(request ? null : false);

  const [step, setStep] = useState<Step>("email");
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [turnstileToken, setTurnstileToken] = useState<string | null>(null);
  const [ticket, setTicket] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const turnstileRef = useRef<TurnstileInstance | null>(null);
  const passkeySupported = browserSupportsWebAuthn();
  const autofillStarted = useRef(false);

  useEffect(() => {
    if (!request) return;
    let cancelled = false;
    graphql<{
      tallySignInRequest: { clientName: string; isDevelopmentBuild: boolean };
    }>(
      `
        query TallySignInRequest($clientId: String!, $redirectUri: String!) {
          tallySignInRequest(clientId: $clientId, redirectUri: $redirectUri) {
            clientName
            isDevelopmentBuild
          }
        }
      `,
      { clientId: request.clientId, redirectUri: request.redirectUri },
    )
      .then((reply) => {
        if (cancelled) return;
        const info = reply.data?.tallySignInRequest;
        if (info) {
          setClientName(info.clientName);
          setIsDevBuild(info.isDevelopmentBuild);
        }
        setLinkOk(!!info);
      })
      .catch(() => {
        if (!cancelled) setLinkOk(false);
      });
    return () => {
      cancelled = true;
    };
  }, [request]);

  // Offer saved passkeys on the email field (the keyboard's suggestions), once
  // the link checks out. Silent if they pick none.
  useEffect(() => {
    if (!linkOk || !request || autofillStarted.current) return;
    autofillStarted.current = true;
    void (async () => {
      if (!(await browserSupportsWebAuthnAutofill())) return;
      await signInWithPasskey(request, true);
    })();
  }, [linkOk, request]);

  if (linkOk === false || !request) return <LinkProblem />;
  if (linkOk === null) return <LoadingIndicator />;

  async function handleSendCode(e: React.FormEvent) {
    e.preventDefault();
    if (!turnstileToken && TURNSTILE_SITE_KEY) return;
    setStep("sending");
    setError(null);
    try {
      await graphql(
        `
          mutation RequestTallySignInCode(
            $email: String!
            $turnstileToken: String!
          ) {
            requestTallySignInCode(
              email: $email
              turnstileToken: $turnstileToken
            )
          }
        `,
        { email, turnstileToken: turnstileToken ?? "" },
      );
      setStep("code");
    } catch {
      setError("We couldn't send a code. Please try again.");
      setStep("email");
      turnstileRef.current?.reset();
      setTurnstileToken(null);
    }
  }

  async function handleVerifyCode(e: React.FormEvent) {
    e.preventDefault();
    setStep("verifying");
    setError(null);
    try {
      const reply = await graphql<{
        redeemTallySignInCode: {
          redirectUrl: string | null;
          passkeyTicket: string | null;
        };
      }>(
        `
          mutation RedeemTallySignInCode(
            $email: String!
            $code: String!
            $request: TallyAuthorizationInput!
            $canSavePasskey: Boolean!
          ) {
            redeemTallySignInCode(
              email: $email
              code: $code
              request: $request
              canSavePasskey: $canSavePasskey
            ) {
              redirectUrl
              passkeyTicket
            }
          }
        `,
        {
          email,
          code,
          request,
          canSavePasskey: await canSavePasskey(email),
        },
      );
      const step = reply.data?.redeemTallySignInCode;
      if (step?.passkeyTicket) {
        setTicket(step.passkeyTicket);
        setStep("offer");
        return;
      }
      if (step?.redirectUrl) {
        setStep("done");
        window.location.assign(step.redirectUrl);
        return;
      }
      setError(
        reply.errors?.[0]?.message.includes("developer account")
          ? "This is a development build of the app. Only a developer account can sign in to it."
          : "That code is wrong or has expired. Check it, or start over for a new one.",
      );
      setStep("code");
      setCode("");
    } catch {
      setError("Something went wrong. Please try again.");
      setStep("code");
    }
  }

  /** Save a passkey with the sign-in ticket, then go back to the app. */
  async function handleSavePasskey(signInTicket: string) {
    setStep("saving");
    setError(null);
    try {
      const begin = await graphql<{
        beginTallyPasskeyRegistration: {
          challengeId: string;
          optionsJson: string;
        };
      }>(
        `
          mutation BeginTallyPasskeyRegistration($ticket: String!) {
            beginTallyPasskeyRegistration(ticket: $ticket) {
              challengeId
              optionsJson
            }
          }
        `,
        { ticket: signInTicket },
      );
      const challenge = begin.data?.beginTallyPasskeyRegistration;
      if (!challenge) {
        setError(
          refusalMessage(begin) ??
            "We couldn't start saving a passkey. Try again, or continue without one.",
        );
        setStep("offer");
        return;
      }
      const created = await createPasskey(challenge.optionsJson);
      if (created.status !== "ok") {
        setError("No passkey was saved. Try again, or continue without one.");
        setStep("offer");
        return;
      }
      const finish = await graphql<{ finishTallyPasskeyRegistration: boolean }>(
        `
          mutation FinishTallyPasskeyRegistration(
            $ticket: String!
            $challengeId: String!
            $credentialJson: String!
          ) {
            finishTallyPasskeyRegistration(
              ticket: $ticket
              challengeId: $challengeId
              credentialJson: $credentialJson
            )
          }
        `,
        {
          ticket: signInTicket,
          challengeId: challenge.challengeId,
          credentialJson: created.credentialJson,
        },
      );
      if (!finish.data?.finishTallyPasskeyRegistration) {
        setError(
          refusalMessage(finish) ??
            "That passkey couldn't be saved. Try again, or continue without one.",
        );
        setStep("offer");
        return;
      }
    } catch {
      setError("Something went wrong. Try again, or continue without one.");
      setStep("offer");
      return;
    }
    await continueSignIn(signInTicket);
  }

  /** Exchange the sign-in ticket for the way back to the app. */
  async function continueSignIn(signInTicket: string) {
    setStep("saving");
    setError(null);
    try {
      const reply = await graphql<{ continueTallySignIn: string }>(
        `
          mutation ContinueTallySignIn($ticket: String!) {
            continueTallySignIn(ticket: $ticket)
          }
        `,
        { ticket: signInTicket },
      );
      const url = reply.data?.continueTallySignIn;
      if (url) {
        setStep("done");
        window.location.assign(url);
        return;
      }
      setError(
        refusalMessage(reply) ?? "Something went wrong. Please start over.",
      );
    } catch {
      setError("Something went wrong. Please try again.");
    }
    setStep("offer");
  }

  /**
   * Sign in with a passkey: from the email field's suggestions (`autofill`), or
   * the button. A cancelled prompt from the field says nothing.
   */
  async function signInWithPasskey(
    authRequest: AuthorizationRequest,
    autofill: boolean,
  ) {
    const assertion = await getPasskeyAssertion({ useAutofill: autofill });
    if (assertion.status !== "ok") {
      if (!autofill) {
        setError(
          assertion.status === "cancelled"
            ? "No passkey was used. Try again, or sign in with your email."
            : "Passkey sign-in failed. Please try again, or sign in with your email.",
        );
      }
      return;
    }
    setStep("passkey");
    setError(null);
    try {
      const reply = await graphql<{ completeTallyPasskeySignIn: string }>(
        `
          mutation CompleteTallyPasskeySignIn(
            $challengeId: String!
            $credentialJson: String!
            $request: TallyAuthorizationInput!
          ) {
            completeTallyPasskeySignIn(
              challengeId: $challengeId
              credentialJson: $credentialJson
              request: $request
            )
          }
        `,
        {
          challengeId: assertion.challengeId,
          credentialJson: assertion.credentialJson,
          request: authRequest,
        },
      );
      const url = reply.data?.completeTallyPasskeySignIn;
      if (url) {
        setStep("done");
        window.location.assign(url);
        return;
      }
      setError(
        refusalMessage(reply) ??
          "Passkey sign-in failed. Please try again, or sign in with your email.",
      );
    } catch {
      setError("Something went wrong. Please try again.");
    }
    setStep("email");
  }

  function startOver() {
    setStep("email");
    setTicket(null);
    setCode("");
    setError(null);
    setTurnstileToken(null);
  }

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Sign in to {clientName}</PanelTitle>

        {isDevBuild && (
          <PanelMessage variant="warning">
            This is a development build of {clientName}. Only a developer
            account can sign in to it.
          </PanelMessage>
        )}

        {(step === "email" || step === "sending") && (
          <form onSubmit={handleSendCode}>
            <PanelIntro>
              Enter the email address SES has for you, and we'll send you a
              sign-in code.
            </PanelIntro>
            {error && <PanelMessage>{error}</PanelMessage>}
            <TextInput
              type="email"
              className="mb-3 box-border block min-w-65 p-3 text-xl"
              placeholder="Email address"
              autoComplete="email webauthn"
              value={email}
              onChange={(e) => {
                setEmail(e.target.value);
                if (e.target.value.length === 0) setTurnstileToken(null);
              }}
              required
              disabled={step !== "email"}
            />
            {/* Only once they start typing, as on the admin sign-in page, so
                Cloudflare doesn't see challenges nobody submits. */}
            {TURNSTILE_SITE_KEY && email.length > 0 && (
              <div className="mb-3">
                <Turnstile
                  ref={turnstileRef}
                  siteKey={TURNSTILE_SITE_KEY}
                  onSuccess={setTurnstileToken}
                  onExpire={() => setTurnstileToken(null)}
                  onError={() => setTurnstileToken(null)}
                />
              </div>
            )}
            <Button
              type="submit"
              size="panel"
              variant="primary"
              disabled={
                !email ||
                (!turnstileToken && !!TURNSTILE_SITE_KEY) ||
                step !== "email"
              }
            >
              {step === "sending" ? "Sending…" : "Send code"}
            </Button>
            {passkeySupported && (
              <div className="mt-4">
                <Button
                  type="button"
                  size="panel"
                  variant="secondary"
                  onClick={() => void signInWithPasskey(request, false)}
                  disabled={step !== "email"}
                >
                  Sign in with a passkey
                </Button>
              </div>
            )}
          </form>
        )}

        {step === "passkey" && <PanelIntro>Signing you in…</PanelIntro>}

        {(step === "offer" || step === "saving") && ticket && (
          <div>
            <PanelIntro>
              <strong>Sign in faster next time?</strong> Save a passkey on this
              device, and next time sign in with Face ID, your fingerprint or
              your screen lock instead of waiting for a code.
            </PanelIntro>
            {error && <PanelMessage>{error}</PanelMessage>}
            <div className="flex flex-row flex-wrap items-center gap-3">
              <Button
                type="button"
                size="panel"
                variant="primary"
                onClick={() => void handleSavePasskey(ticket)}
                disabled={step !== "offer"}
              >
                {step === "saving" ? "Saving…" : "Save a passkey"}
              </Button>
              <Button
                type="button"
                size="panel"
                variant="secondary"
                onClick={() => {
                  dismissOffer(email);
                  void continueSignIn(ticket);
                }}
                disabled={step !== "offer"}
              >
                Not now
              </Button>
            </div>
            {error?.includes("timed out") && (
              <div className="mt-4">
                <Button
                  type="button"
                  size="panel"
                  variant="secondary"
                  onClick={startOver}
                >
                  Start over
                </Button>
              </div>
            )}
          </div>
        )}

        {(step === "code" || step === "verifying") && (
          <form onSubmit={handleVerifyCode}>
            <PanelIntro>
              If <strong>{email}</strong> can sign in to {clientName}, we've
              sent it a 6-digit code. Enter it below.
            </PanelIntro>
            {error && <PanelMessage>{error}</PanelMessage>}
            <TextInput
              type="text"
              inputMode="numeric"
              autoComplete="one-time-code"
              pattern="[0-9]{6}"
              maxLength={6}
              className="mb-3 box-border block min-w-65 p-3 text-xl"
              placeholder="6-digit code"
              aria-label="6-digit code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              required
              autoFocus
              disabled={step !== "code"}
            />
            <div className="flex flex-row flex-wrap items-center gap-3">
              <Button
                type="submit"
                size="panel"
                variant="primary"
                disabled={code.length !== 6 || step !== "code"}
              >
                {step === "verifying" ? "Checking…" : "Sign in"}
              </Button>
              <Button
                type="button"
                size="panel"
                variant="secondary"
                onClick={startOver}
              >
                Start over
              </Button>
            </div>
            <Muted className="mt-4">
              No code? Check the address, or ask your unit to check the email
              SES has for you.
            </Muted>
          </form>
        )}

        {step === "done" && (
          <PanelIntro>Signed in. Returning you to {clientName}…</PanelIntro>
        )}
      </PanelBox>
    </Panel>
  );
}
