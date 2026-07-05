import { Turnstile, type TurnstileInstance } from "@marsidev/react-turnstile";
import { useState, useRef, useEffect } from "react";
import { getGraphQLEndpoint } from "../../lib/api";
import {
  getCurrentClientVersion,
  CLIENT_VERSION_HEADER,
} from "../../lib/clientVersion";
import {
  loginWithPasskey,
  browserSupportsWebAuthn,
  browserSupportsWebAuthnAutofill,
} from "../../lib/passkey";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
  PanelMessage,
} from "../../components/ui/Panel";
import { Button } from "../../components/ui/Button";
import TextInput from "../../components/ui/TextInput";

interface AdminLoginPageProps {
  errorMessage?: string | null;
  onNewTokenReceived: (token: string) => void;
}

const TURNSTILE_SITE_KEY =
  import.meta.env.VITE_TURNSTILE_DISABLED === "1"
    ? ""
    : (import.meta.env.VITE_TURNSTILE_SITE_KEY ?? "");

type EmailCodeStep = "idle" | "sending" | "awaiting_code" | "verifying";

async function callMutation(
  query: string,
  variables: Record<string, string>,
): Promise<unknown> {
  const resp = await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      [CLIENT_VERSION_HEADER]: getCurrentClientVersion(),
    },
    body: JSON.stringify({ query, variables }),
    cache: "no-store",
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  return await resp.json();
}

export default function AdminLoginPage({
  errorMessage,
  onNewTokenReceived,
}: AdminLoginPageProps) {
  const [step, setStep] = useState<EmailCodeStep>("idle");
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [turnstileToken, setTurnstileToken] = useState<string | null>(null);
  const [codeError, setCodeError] = useState<string | null>(null);
  const [passkeySigningIn, setPasskeySigningIn] = useState(false);
  const [passkeyError, setPasskeyError] = useState<string | null>(null);
  const turnstileRef = useRef<TurnstileInstance | null>(null);
  const passkeySupported = browserSupportsWebAuthn();
  const onNewTokenReceivedRef = useRef(onNewTokenReceived);
  useEffect(() => {
    onNewTokenReceivedRef.current = onNewTokenReceived;
  }, [onNewTokenReceived]);

  // Ensures the autofill ceremony is started only once. StrictMode (dev) runs
  // effects setup→cleanup→setup on the same instance, so without this guard we
  // fire beginPasskeyLogin twice and end up with two competing ceremonies.
  const autofillStartedRef = useRef(false);
  // Tracks whether the component is really mounted. StrictMode's fake unmount
  // flips this false, but its second setup flips it back true — so a token from
  // the single ceremony isn't discarded, while a real unmount still discards.
  const mountedRef = useRef(true);

  // Transparent passkey login: when the page loads, prime a discoverable
  // challenge and attach it to the email field via browser autofill. If the
  // user picks a saved passkey we log them straight in; otherwise this is a
  // no-op and the email-code flow remains available.
  useEffect(() => {
    mountedRef.current = true;
    if (autofillStartedRef.current) return;
    autofillStartedRef.current = true;
    (async () => {
      try {
        if (!(await browserSupportsWebAuthnAutofill())) return;
        const result = await loginWithPasskey({ useAutofill: true });
        if (!mountedRef.current) return;
        if (result.status === "ok") {
          onNewTokenReceivedRef.current(result.token);
        } else if (result.status === "failed") {
          setPasskeyError("Passkey login failed.");
        }
        // "cancelled" → stay silent (user dismissed the autofill prompt).
      } catch (err) {
        // Conditional UI unsupported or aborted — ignore silently.
        console.warn("[passkey] autofill effect threw:", err);
      }
    })();
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // Manual passkey sign-in (modal prompt) — fallback for browsers where the
  // autofill/conditional-UI path doesn't surface saved passkeys.
  async function handlePasskeyLogin() {
    setPasskeySigningIn(true);
    setPasskeyError(null);
    try {
      const result = await loginWithPasskey({ useAutofill: false });
      if (result.status === "ok") {
        onNewTokenReceived(result.token);
      } else if (result.status === "failed") {
        setPasskeyError("Passkey login failed.");
      } else {
        // "cancelled" — the user dismissed the prompt or has no passkey.
        setPasskeyError(
          "No passkey was used. Make sure you've added one, or sign in another way.",
        );
      }
    } catch (err) {
      console.warn("[passkey] manual button threw:", err);
      setPasskeyError("Passkey sign-in failed. Please try again.");
    } finally {
      setPasskeySigningIn(false);
    }
  }

  async function handleSendCode(e: React.FormEvent) {
    e.preventDefault();
    if (!turnstileToken && TURNSTILE_SITE_KEY) return;
    setStep("sending");
    setCodeError(null);
    try {
      await callMutation(
        `mutation RequestAuthCode($email: String!, $turnstileToken: String!) {
          requestAuthCode(email: $email, turnstileToken: $turnstileToken)
        }`,
        { email, turnstileToken: turnstileToken ?? "" },
      );
      setStep("awaiting_code");
    } catch {
      setCodeError("Failed to send code. Please try again.");
      setStep("idle");
      turnstileRef.current?.reset();
      setTurnstileToken(null);
    }
  }

  async function handleVerifyCode(e: React.FormEvent) {
    e.preventDefault();
    setStep("verifying");
    setCodeError(null);
    try {
      const result = (await callMutation(
        `mutation VerifyAuthCode($email: String!, $code: String!) {
          verifyAuthCode(email: $email, code: $code)
        }`,
        { email, code },
      )) as { data?: { verifyAuthCode?: string | null } };

      const token = result?.data?.verifyAuthCode;
      if (token) {
        onNewTokenReceived(token);
      } else {
        setCodeError("Incorrect or expired code. Please try again.");
        setStep("awaiting_code");
        setCode("");
      }
    } catch {
      setCodeError("Verification failed. Please try again.");
      setStep("awaiting_code");
    }
  }

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Please sign in to continue</PanelTitle>

        {errorMessage ? <PanelMessage>{errorMessage}</PanelMessage> : null}

        {/* Manual passkey login (fallback when autofill doesn't surface it) */}
        {passkeySupported && (
          <Button
            size="panel"
            onClick={handlePasskeyLogin}
            disabled={passkeySigningIn}
          >
            {passkeySigningIn
              ? "Waiting for passkey..."
              : "Sign in via passkey"}
          </Button>
        )}

        {passkeyError && <PanelMessage>{passkeyError}</PanelMessage>}

        {/* Divider — only when there's a button above it (passkey) */}
        {passkeySupported && (
          <div className="my-6 text-center text-[#888]">— or —</div>
        )}

        {/* New email-code login */}
        {step === "idle" && (
          <form onSubmit={handleSendCode}>
            <PanelIntro>
              Enter your registered email address to receive a login code.
            </PanelIntro>
            {codeError && <PanelMessage>{codeError}</PanelMessage>}
            <TextInput
              type="email"
              className="mb-3 box-border block min-w-65 p-3 text-xl"
              placeholder="Email address"
              autoComplete="username webauthn"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              disabled={step !== "idle"}
            />
            {TURNSTILE_SITE_KEY && (
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
              variant="secondary"
              disabled={
                !email ||
                (!turnstileToken && !!TURNSTILE_SITE_KEY) ||
                step !== "idle"
              }
            >
              Send code
            </Button>
          </form>
        )}

        {step === "sending" && <p>Sending code to {email}…</p>}

        {step === "awaiting_code" && (
          <form onSubmit={handleVerifyCode}>
            <PanelIntro>
              If <strong>{email}</strong> was valid and registered for access to
              the system, a 6-digit code has been sent. Enter it below to login.
            </PanelIntro>
            {codeError && <PanelMessage>{codeError}</PanelMessage>}
            <TextInput
              type="text"
              inputMode="numeric"
              pattern="[0-9]{6}"
              maxLength={6}
              className="mb-3 box-border block min-w-65 p-3 text-xl"
              placeholder="6-digit code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              required
              autoFocus
            />
            <Button
              type="submit"
              size="panel"
              variant="secondary"
              disabled={code.length !== 6}
            >
              Verify code
            </Button>
            <Button
              type="button"
              size="panel"
              variant="secondary"
              className="mt-3"
              onClick={() => {
                setStep("idle");
                setCode("");
                setCodeError(null);
                setTurnstileToken(null);
              }}
            >
              Start over
            </Button>
          </form>
        )}

        {step === "verifying" && <p>Verifying…</p>}
      </PanelBox>
    </Panel>
  );
}
