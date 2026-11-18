import { useEffect, useState } from "react";
import { Navigate, useLocation, useNavigate } from "react-router";

import LoadingIndicator from "../components/LoadingIndicator";
import {
  Panel,
  PanelBox,
  PanelIntro,
  PanelMessage,
  PanelTitle,
} from "../components/ui/Panel";
import { Button } from "../components/ui/Button";
import { StatusMessage } from "../components/ui/StatusMessage";
import {
  isUnauthenticated,
  memberGraphQL,
  readFragment,
  signInPath,
} from "./api";

interface Settings {
  firstName: string;
  lastName: string;
  email: string | null;
  signedInReminders: boolean;
}

const FIELDS = "firstName lastName email signedInReminders";

/**
 * A member's own settings page, at `/member/settings#token=...&email=...`. The
 * `slms_` token comes from the link in a reminder email (good for two weeks) or
 * from signing in at `/member/sign-in`; once it stops working, the member is sent
 * to sign in again with their email filled in.
 *
 * Keyed on the fragment, so opening a newer link in a tab already showing the
 * page (which changes only the fragment) starts afresh with that link's token.
 */
export default function MemberSettings() {
  const { hash } = useLocation();
  return <SettingsPage key={hash} hash={hash} />;
}

function SettingsPage({ hash }: { hash: string }) {
  const navigate = useNavigate();
  const fragment = readFragment(hash);
  const token = fragment.get("token")?.trim() ?? "";
  const email = fragment.get("email") ?? "";

  const [settings, setSettings] = useState<Settings | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<{
    variant: "success" | "error";
    text: string;
  } | null>(null);

  useEffect(() => {
    if (!token) return;
    let cancelled = false;
    memberGraphQL<{ memberSettings: Settings }>(
      `
        query MemberSettings($token: String!) {
          memberSettings(token: $token) { ${FIELDS} }
        }
      `,
      { token },
    )
      .then((reply) => {
        if (cancelled) return;
        if (reply.data?.memberSettings) {
          setSettings(reply.data.memberSettings);
        } else if (isUnauthenticated(reply)) {
          void navigate(signInPath(email, true), { replace: true });
        } else {
          setLoadFailed(true);
        }
      })
      .catch(() => {
        if (!cancelled) setLoadFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [token, email, navigate]);

  if (!token) return <Navigate to={signInPath(email, false)} replace />;

  if (loadFailed) {
    return (
      <Panel>
        <PanelBox>
          <PanelTitle>Member email settings</PanelTitle>
          <PanelMessage>
            We couldn't load your settings. Please reload the page to try again.
          </PanelMessage>
        </PanelBox>
      </Panel>
    );
  }

  if (!settings) return <LoadingIndicator />;

  async function save(signedInReminders: boolean) {
    setSaving(true);
    setStatus(null);
    try {
      const reply = await memberGraphQL<{ updateMemberSettings: Settings }>(
        `
          mutation UpdateMemberSettings(
            $token: String!
            $signedInReminders: Boolean!
          ) {
            updateMemberSettings(
              token: $token
              signedInReminders: $signedInReminders
            ) { ${FIELDS} }
          }
        `,
        { token, signedInReminders },
      );
      if (reply.data?.updateMemberSettings) {
        setSettings(reply.data.updateMemberSettings);
        setStatus({ variant: "success", text: "Saved." });
      } else if (isUnauthenticated(reply)) {
        void navigate(signInPath(email, true), { replace: true });
      } else {
        setStatus({
          variant: "error",
          text: "Your change wasn't saved. Please try again.",
        });
      }
    } catch {
      setStatus({
        variant: "error",
        text: "Your change wasn't saved. Please try again.",
      });
    }
    setSaving(false);
  }

  async function signOut() {
    try {
      await memberGraphQL(
        `
          mutation SignOutOfMemberSettings($token: String!) {
            signOutOfMemberSettings(token: $token)
          }
        `,
        { token },
      );
    } catch {
      // Leaving the page is what matters; the token expires by itself.
    }
    void navigate(signInPath(email, false), { replace: true });
  }

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Member email settings</PanelTitle>
        <PanelIntro>
          Signed in as{" "}
          <strong>
            {settings.firstName} {settings.lastName}
          </strong>
          {settings.email && <> ({settings.email})</>}.
        </PanelIntro>

        <label className="mb-2 flex items-start gap-3 text-lg">
          <input
            type="checkbox"
            className="mt-1.5 size-5 shrink-0"
            checked={settings.signedInReminders}
            disabled={saving}
            onChange={(e) => void save(e.target.checked)}
          />
          <span>
            <strong>Remind me when I forget to sign out</strong>
            <br />
            Your unit can email you when you're still signed in after you've
            left, with a link to finish signing out. Untick this to stop those
            emails.
          </span>
        </label>
        <div className="mb-6 min-h-7">
          {saving && <StatusMessage variant="success">Saving…</StatusMessage>}
          {!saving && status && (
            <StatusMessage variant={status.variant}>
              {status.text}
            </StatusMessage>
          )}
        </div>

        <Button
          type="button"
          size="panel"
          variant="secondary"
          className="w-full"
          onClick={() => void signOut()}
        >
          Sign out
        </Button>
      </PanelBox>
    </Panel>
  );
}
