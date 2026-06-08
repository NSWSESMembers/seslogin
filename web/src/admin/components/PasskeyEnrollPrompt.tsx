import { useEffect, useState, type ReactNode } from "react";
import { graphql, useLazyLoadQuery } from "react-relay";
import {
  browserSupportsWebAuthn,
  wasPasskeyLoginSession,
  passkeyEnrollPromptThrottled,
  markPasskeyEnrollPromptShown,
} from "../../lib/passkey";
import { usePasskeyRegistration } from "./usePasskeyRegistration";
import { useNotify } from "./useNotify";
import { getErrorMessage } from "../../lib/relayErrors";
import type { PasskeyEnrollPromptQuery } from "./__generated__/PasskeyEnrollPromptQuery.graphql";

/**
 * Gate shown right after sign-in if the user has no passkey yet. Renders a
 * full-screen interstitial (styled like the login window) over the app, at most
 * once every 12 hours. Suppressed when the device lacks WebAuthn or when the
 * current session was itself authenticated via passkey.
 */
export default function PasskeyEnrollPrompt({
  children,
}: {
  children: ReactNode;
}) {
  const data = useLazyLoadQuery<PasskeyEnrollPromptQuery>(
    graphql`
      query PasskeyEnrollPromptQuery {
        user {
          passkeys {
            __typename
          }
        }
      }
    `,
    {},
  );

  const hasPasskey = data.user.passkeys.length > 0;

  const [show, setShow] = useState(
    () =>
      !hasPasskey &&
      browserSupportsWebAuthn() &&
      !wasPasskeyLoginSession() &&
      !passkeyEnrollPromptThrottled(),
  );

  useEffect(() => {
    if (show) markPasskeyEnrollPromptShown();
  }, [show]);

  if (!show) return <>{children}</>;

  return <PasskeyInterstitial onDone={() => setShow(false)} />;
}

function PasskeyInterstitial({ onDone }: { onDone: () => void }) {
  const register = usePasskeyRegistration();
  const { notifySuccess } = useNotify();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleEnroll() {
    setBusy(true);
    setError(null);
    try {
      await register(defaultPasskeyName());
      notifySuccess("Passkey added");
      onDone();
    } catch (err) {
      setError(`Couldn't add a passkey: ${getErrorMessage(err)}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="action-panel">
      <div className="action-panel__panel">
        <h1>Add a passkey</h1>
        <p className="action-panel__intro">
          Skip the email codes — sign in with Face ID, Touch ID, or your device
          PIN. It only takes a moment to set up.
        </p>

        <ul className="passkey-benefits">
          <li>
            <strong>Faster than an email code.</strong> No waiting for a message
            to arrive — unlock with your face, fingerprint, or PIN and
            you&apos;re in.
          </li>
          <li>
            <strong>More secure than a password.</strong> Passkeys can&apos;t be
            guessed, reused, or phished, and never leave your device.
          </li>
        </ul>

        {error && (
          <div className="action-panel__message action-panel__message--error">
            {error}
          </div>
        )}

        <button
          type="button"
          className="action-button action-panel__button"
          onClick={handleEnroll}
          disabled={busy}
        >
          {busy ? "Setting up…" : "Add a passkey"}
        </button>

        <div style={{ marginTop: "18px" }}>
          <button
            type="button"
            onClick={onDone}
            disabled={busy}
            style={{
              background: "none",
              border: "none",
              padding: 0,
              color: "#7a6a5d",
              font: "inherit",
              fontSize: "15px",
              textDecoration: "underline",
              cursor: "pointer",
            }}
          >
            Maybe later
          </button>
        </div>
      </div>
    </section>
  );
}

function defaultPasskeyName(): string {
  const ua = navigator.userAgent;
  let device = "This device";
  if (/iPhone/.test(ua)) device = "iPhone";
  else if (/iPad/.test(ua)) device = "iPad";
  else if (/Macintosh/.test(ua)) device = "Mac";
  else if (/Android/.test(ua)) device = "Android";
  else if (/Windows/.test(ua)) device = "Windows";
  return device;
}
