import { useEffect, useState } from "react";

import { getGraphQLEndpoint } from "../lib/api";
import { clientHeaders } from "../lib/clientInfo";
import { browserSupportsWebAuthn, createPasskey } from "../lib/passkey";
import {
  Panel,
  PanelBox,
  PanelIntro,
  PanelMessage,
  PanelTitle,
} from "../components/ui/Panel";
import { Button } from "../components/ui/Button";
import TextInput from "../components/ui/TextInput";

interface GraphQLReply<T> {
  data?: T | null;
  errors?: { message: string; extensions?: { code?: string } }[];
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

/** The server's own words for a refusal it words for people, or null. */
function refusalMessage(reply: GraphQLReply<unknown>): string | null {
  const err = reply.errors?.[0];
  const code = err?.extensions?.code;
  return err && (code === "BAD_REQUEST" || code === "FORBIDDEN")
    ? err.message
    : null;
}

/** The ticket Tally opened this page with, from the URL fragment (never sent to a server). */
function readTicket(): string | null {
  return window.location.hash.replace(/^#/, "") || null;
}

type Step =
  "email" | "sending" | "code" | "verifying" | "ready" | "saving" | "done";

/**
 * Tally's "Add a passkey" page, at `/tally/passkey#<ticket>`: Tally's settings
 * open it with a ticket from `startTallyPasskeySetup`. It confirms an email with
 * a code first, then saves a passkey on this device (the SESLogin user's if the
 * email is one, else the member's) and sends the browser back to the app. See
 * `api/src/graphql/tally_passkeys.rs`.
 */
export default function TallyAddPasskey() {
  const [ticket] = useState(readTicket);
  const [step, setStep] = useState<Step>("email");
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const supported = browserSupportsWebAuthn();

  // Clear the ticket from the address, so it isn't left in the history.
  useEffect(() => {
    if (!window.location.hash) return;
    try {
      window.history.replaceState(
        null,
        "",
        window.location.pathname + window.location.search,
      );
    } catch {
      // Not fatal: the ticket is single use and short-lived anyway.
    }
  }, []);

  if (!ticket) {
    return (
      <Panel>
        <PanelBox>
          <PanelTitle>Can't add a passkey here</PanelTitle>
          <PanelIntro>
            Open Tally and add a passkey from Profile › Passkeys.
          </PanelIntro>
        </PanelBox>
      </Panel>
    );
  }

  async function sendCode(setupTicket: string) {
    setStep("sending");
    setError(null);
    try {
      const reply = await graphql<{ sendTallyPasskeySetupCode: boolean }>(
        `
          mutation SendTallyPasskeySetupCode(
            $ticket: String!
            $email: String!
          ) {
            sendTallyPasskeySetupCode(ticket: $ticket, email: $email)
          }
        `,
        { ticket: setupTicket, email },
      );
      if (!reply.data?.sendTallyPasskeySetupCode) {
        setError(
          refusalMessage(reply) ?? "We couldn't send a code. Please try again.",
        );
        setStep("email");
        return;
      }
      setCode("");
      setStep("code");
    } catch {
      setError("We couldn't send a code. Please try again.");
      setStep("email");
    }
  }

  async function verifyCode(setupTicket: string) {
    setStep("verifying");
    setError(null);
    try {
      const reply = await graphql<{ verifyTallyPasskeySetupCode: boolean }>(
        `
          mutation VerifyTallyPasskeySetupCode(
            $ticket: String!
            $email: String!
            $code: String!
          ) {
            verifyTallyPasskeySetupCode(
              ticket: $ticket
              email: $email
              code: $code
            )
          }
        `,
        { ticket: setupTicket, email, code },
      );
      if (!reply.data?.verifyTallyPasskeySetupCode) {
        setError(
          refusalMessage(reply) ??
            "That code is wrong or has expired. Check it, or send a new one.",
        );
        setCode("");
        setStep("code");
        return;
      }
      setStep("ready");
    } catch {
      setError("Something went wrong. Please try again.");
      setStep("code");
    }
  }

  async function save(setupTicket: string) {
    setStep("saving");
    setError(null);
    try {
      const begin = await graphql<{
        beginTallyPasskeySetup: { challengeId: string; optionsJson: string };
      }>(
        `
          mutation BeginTallyPasskeySetup($ticket: String!) {
            beginTallyPasskeySetup(ticket: $ticket) {
              challengeId
              optionsJson
            }
          }
        `,
        { ticket: setupTicket },
      );
      const challenge = begin.data?.beginTallyPasskeySetup;
      if (!challenge) {
        setError(
          refusalMessage(begin) ??
            "We couldn't start adding a passkey. Please try again.",
        );
        setStep("ready");
        return;
      }
      const created = await createPasskey(challenge.optionsJson);
      if (created.status !== "ok") {
        setError("No passkey was saved. Please try again.");
        setStep("ready");
        return;
      }
      const finish = await graphql<{ finishTallyPasskeySetup: string }>(
        `
          mutation FinishTallyPasskeySetup(
            $ticket: String!
            $challengeId: String!
            $credentialJson: String!
          ) {
            finishTallyPasskeySetup(
              ticket: $ticket
              challengeId: $challengeId
              credentialJson: $credentialJson
            )
          }
        `,
        {
          ticket: setupTicket,
          challengeId: challenge.challengeId,
          credentialJson: created.credentialJson,
        },
      );
      const url = finish.data?.finishTallyPasskeySetup;
      if (!url) {
        setError(
          refusalMessage(finish) ??
            "That passkey couldn't be saved. Please try again.",
        );
        setStep("ready");
        return;
      }
      setStep("done");
      window.location.assign(url);
    } catch {
      setError("Something went wrong. Please try again.");
      setStep("ready");
    }
  }

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Add a passkey to Tally</PanelTitle>
        {!supported ? (
          <PanelIntro>
            This browser can't save passkeys. Go back to Tally to keep signing
            in with a code sent to your email.
          </PanelIntro>
        ) : step === "email" || step === "sending" ? (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void sendCode(ticket);
            }}
          >
            <PanelIntro>
              First, confirm it's you. Enter your service email address, or
              another email address registered in SESLogin, and we'll send it a
              code. If it's your SESLogin email, the passkey will sign in to
              SESLogin too.
            </PanelIntro>
            {error && <PanelMessage>{error}</PanelMessage>}
            <TextInput
              type="email"
              className="mb-3 box-border block w-full p-3 text-xl"
              placeholder="Email address"
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              disabled={step !== "email"}
            />
            <Button
              type="submit"
              size="panel"
              variant="primary"
              className="w-full"
              disabled={!email || step !== "email"}
            >
              {step === "sending" ? "Sending…" : "Send code"}
            </Button>
          </form>
        ) : step === "code" || step === "verifying" ? (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void verifyCode(ticket);
            }}
          >
            <PanelIntro>
              If <strong>{email}</strong> is yours, we've sent it a 6-digit
              code. Enter it below.
            </PanelIntro>
            {error && <PanelMessage>{error}</PanelMessage>}
            <TextInput
              type="text"
              inputMode="numeric"
              autoComplete="one-time-code"
              pattern="[0-9]{6}"
              maxLength={6}
              className="mb-3 box-border block w-full p-3 text-xl"
              placeholder="6-digit code"
              aria-label="6-digit code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              required
              autoFocus
              disabled={step !== "code"}
            />
            <div className="flex flex-col gap-3">
              <Button
                type="submit"
                size="panel"
                variant="primary"
                className="w-full"
                disabled={code.length !== 6 || step !== "code"}
              >
                {step === "verifying" ? "Checking…" : "Confirm"}
              </Button>
              <Button
                type="button"
                size="panel"
                variant="secondary"
                className="w-full"
                onClick={() => {
                  setError(null);
                  setStep("email");
                }}
              >
                Use a different email
              </Button>
            </div>
          </form>
        ) : step === "done" ? (
          <PanelIntro>Passkey saved. Returning you to Tally…</PanelIntro>
        ) : (
          <div>
            <PanelIntro>
              Email confirmed. Save a passkey on this device, and sign in with
              Face ID, your fingerprint or your screen lock instead of waiting
              for a code.
            </PanelIntro>
            {error && <PanelMessage>{error}</PanelMessage>}
            <Button
              type="button"
              size="panel"
              variant="primary"
              className="w-full"
              onClick={() => void save(ticket)}
              disabled={step !== "ready"}
            >
              {step === "saving" ? "Saving…" : "Add a passkey"}
            </Button>
          </div>
        )}
      </PanelBox>
    </Panel>
  );
}
