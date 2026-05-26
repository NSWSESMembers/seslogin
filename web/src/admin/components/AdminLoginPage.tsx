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

interface AdminLoginPageProps {
  /**
   * Triggers the Auth0 popup sign-in. Currently not rendered (the Auth0 button
   * was removed) but kept wired up so it can be re-enabled easily.
   */
  onLogin: () => void | Promise<void>;
  onLogout?: () => void | Promise<void>;
  isLoading?: boolean;
  errorMessage?: string | null;
  showUnauthorizedMessage?: boolean;
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
  // onLogin (Auth0) intentionally not consumed — the button was removed.
  onLogout,
  isLoading = false,
  errorMessage,
  showUnauthorizedMessage = false,
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
  // no-op and the email-code / Auth0 flows remain available.
  useEffect(() => {
    mountedRef.current = true;
    if (showUnauthorizedMessage) return;
    if (autofillStartedRef.current) return;
    autofillStartedRef.current = true;
    (async () => {
      try {
        if (!(await browserSupportsWebAuthnAutofill())) return;
        const token = await loginWithPasskey({ useAutofill: true });
        if (token && mountedRef.current) {
          onNewTokenReceivedRef.current(token);
        }
      } catch (err) {
        // Conditional UI unsupported or aborted — ignore silently.
        console.warn("[passkey] autofill effect threw:", err);
      }
    })();
    return () => {
      mountedRef.current = false;
    };
  }, [showUnauthorizedMessage]);

  // Manual passkey sign-in (modal prompt) — fallback for browsers where the
  // autofill/conditional-UI path doesn't surface saved passkeys.
  async function handlePasskeyLogin() {
    setPasskeySigningIn(true);
    setPasskeyError(null);
    try {
      const token = await loginWithPasskey({ useAutofill: false });
      if (token) {
        onNewTokenReceived(token);
      } else {
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
    <section className="action-panel">
      <div className="action-panel__panel">
        <h1>Please sign in to continue</h1>

        {showUnauthorizedMessage ? (
          <div className="action-panel__message action-panel__message--warning">
            An unauthorized error was encountered. It is possible that the email
            address you used to sign in is not registered for admin access. Try
            refreshing the page if you believe you used the correct email
            address or click the button below to try logging in with a different
            email address.
          </div>
        ) : null}

        {errorMessage ? (
          <div className="action-panel__message action-panel__message--error">
            {errorMessage}
          </div>
        ) : null}

        {/*
          Auth0 sign-in button removed (handler `onLogin` retained for now).
          The Auth0 flow still works and can be re-enabled by rendering a
          button wired to onLogin.
        */}

        {/* Manual passkey login (fallback when autofill doesn't surface it) */}
        {!showUnauthorizedMessage && passkeySupported && (
          <button
            type="button"
            className="action-button action-panel__button"
            onClick={handlePasskeyLogin}
            disabled={passkeySigningIn || isLoading}
          >
            {passkeySigningIn
              ? "Waiting for passkey..."
              : "Sign in via passkey"}
          </button>
        )}

        {passkeyError && (
          <div className="action-panel__message action-panel__message--error">
            {passkeyError}
          </div>
        )}

        {showUnauthorizedMessage && onLogout ? (
          <button
            type="button"
            className="action-button action-panel__button action-panel__button--secondary"
            onClick={onLogout}
            disabled={isLoading}
          >
            Log out of current account
          </button>
        ) : null}

        {/* Divider — only when there's a button above it (passkey) */}
        {!showUnauthorizedMessage && passkeySupported && (
          <div
            style={{ margin: "1.5rem 0", textAlign: "center", color: "#888" }}
          >
            — or —
          </div>
        )}

        {/* New email-code login */}
        {!showUnauthorizedMessage && step === "idle" && (
          <form onSubmit={handleSendCode}>
            <p className="action-panel__intro">
              Enter your registered email address to receive a login code.
            </p>
            {codeError && (
              <div className="action-panel__message action-panel__message--error">
                {codeError}
              </div>
            )}
            <input
              type="email"
              className="action-panel__input"
              placeholder="Email address"
              autoComplete="username webauthn"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              disabled={step !== "idle"}
              style={{
                display: "block",
                width: "100%",
                marginBottom: "0.75rem",
              }}
            />
            {TURNSTILE_SITE_KEY && (
              <div style={{ marginBottom: "0.75rem" }}>
                <Turnstile
                  ref={turnstileRef}
                  siteKey={TURNSTILE_SITE_KEY}
                  onSuccess={setTurnstileToken}
                  onExpire={() => setTurnstileToken(null)}
                  onError={() => setTurnstileToken(null)}
                />
              </div>
            )}
            <button
              type="submit"
              className="action-button action-panel__button action-panel__button--secondary"
              disabled={
                !email ||
                (!turnstileToken && !!TURNSTILE_SITE_KEY) ||
                step !== "idle"
              }
            >
              Send code
            </button>
          </form>
        )}

        {!showUnauthorizedMessage && step === "sending" && (
          <p>Sending code to {email}…</p>
        )}

        {!showUnauthorizedMessage && step === "awaiting_code" && (
          <form onSubmit={handleVerifyCode}>
            <p className="action-panel__intro">
              A 6-digit code was sent to <strong>{email}</strong>. Enter it
              below.
            </p>
            {codeError && (
              <div className="action-panel__message action-panel__message--error">
                {codeError}
              </div>
            )}
            <input
              type="text"
              inputMode="numeric"
              pattern="[0-9]{6}"
              maxLength={6}
              className="action-panel__input"
              placeholder="6-digit code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              required
              autoFocus
              style={{
                display: "block",
                width: "100%",
                marginBottom: "0.75rem",
              }}
            />
            <button
              type="submit"
              className="action-button action-panel__button action-panel__button--secondary"
              disabled={code.length !== 6}
            >
              Verify code
            </button>
            <button
              type="button"
              className="action-button action-panel__button action-panel__button--secondary"
              style={{ marginTop: "0.5rem" }}
              onClick={() => {
                setStep("idle");
                setCode("");
                setCodeError(null);
                setTurnstileToken(null);
              }}
            >
              Start over
            </button>
          </form>
        )}

        {!showUnauthorizedMessage && step === "verifying" && <p>Verifying…</p>}
      </div>
    </section>
  );
}
