import { Turnstile, type TurnstileInstance } from "@marsidev/react-turnstile";
import { useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router";

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
import { memberGraphQL, readFragment, settingsPath } from "./api";

const TURNSTILE_SITE_KEY =
  import.meta.env.VITE_TURNSTILE_DISABLED === "1"
    ? ""
    : (import.meta.env.VITE_TURNSTILE_SITE_KEY ?? "");

type Step = "email" | "sending" | "code" | "verifying";

/**
 * The member settings sign-in page, at `/member/sign-in`: a member types their
 * service email, is emailed a code, and lands on their settings page. Nothing to
 * do with the admin sign-in: it never signs anyone in to the admin site.
 *
 * `#email=...` fills the email in (the settings page sends an expired link
 * here with it), and `&expired=1` says why they're here.
 */
export default function MemberSignIn() {
  const { hash } = useLocation();
  const navigate = useNavigate();
  const [fragment] = useState(() => readFragment(hash));
  const expired = fragment.get("expired") === "1";

  const [step, setStep] = useState<Step>("email");
  const [email, setEmail] = useState(() => fragment.get("email") ?? "");
  const [code, setCode] = useState("");
  const [turnstileToken, setTurnstileToken] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const turnstileRef = useRef<TurnstileInstance | null>(null);

  async function handleSendCode(e: React.FormEvent) {
    e.preventDefault();
    if (!turnstileToken && TURNSTILE_SITE_KEY) return;
    setStep("sending");
    setError(null);
    try {
      await memberGraphQL(
        `
          mutation RequestMemberSettingsCode(
            $email: String!
            $turnstileToken: String!
          ) {
            requestMemberSettingsCode(
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
      const reply = await memberGraphQL<{ signInToMemberSettings: string }>(
        `
          mutation SignInToMemberSettings($email: String!, $code: String!) {
            signInToMemberSettings(email: $email, code: $code)
          }
        `,
        { email, code },
      );
      const token = reply.data?.signInToMemberSettings;
      if (token) {
        void navigate(settingsPath(token, email.trim().toLowerCase()), {
          replace: true,
        });
        return;
      }
      setError(
        "That code is wrong or has expired. Check it, or start over for a new one.",
      );
      setCode("");
    } catch {
      setError("Something went wrong. Please try again.");
    }
    setStep("code");
  }

  function startOver() {
    setStep("email");
    setCode("");
    setError(null);
    setTurnstileToken(null);
  }

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Member email settings</PanelTitle>

        {(step === "email" || step === "sending") && (
          <form onSubmit={handleSendCode}>
            {expired && (
              <PanelMessage variant="warning">
                That link has expired. Sign in again to manage your email
                settings.
              </PanelMessage>
            )}
            {error && <PanelMessage>{error}</PanelMessage>}
            <PanelIntro>
              Enter your service email address to receive a sign-in code.
            </PanelIntro>
            <TextInput
              type="email"
              className="mb-3 box-border block w-full p-3 text-xl"
              placeholder="Email address"
              aria-label="Email address"
              autoComplete="email"
              value={email}
              onChange={(e) => {
                setEmail(e.target.value);
                if (e.target.value.length === 0) setTurnstileToken(null);
              }}
              required
              disabled={step !== "email"}
            />
            {/* Only once there's an email, as on the other sign-in pages, so
                Cloudflare doesn't see challenges nobody submits. */}
            {TURNSTILE_SITE_KEY && email.length > 0 && (
              <div className="mb-3 flex justify-center">
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
              className="w-full"
              disabled={
                !email ||
                (!turnstileToken && !!TURNSTILE_SITE_KEY) ||
                step !== "email"
              }
            >
              {step === "sending" ? "Sending…" : "Send code"}
            </Button>
          </form>
        )}

        {(step === "code" || step === "verifying") && (
          <form onSubmit={handleVerifyCode}>
            <PanelIntro>
              If <strong>{email}</strong> is a member's service email, we've
              sent it a 6-digit code. Enter it below.
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
                {step === "verifying" ? "Checking…" : "Sign in"}
              </Button>
              <Button
                type="button"
                size="panel"
                variant="secondary"
                className="w-full"
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
      </PanelBox>
    </Panel>
  );
}
