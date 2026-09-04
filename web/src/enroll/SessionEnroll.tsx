import { graphql, useMutation } from "react-relay";
import { Link, useSearchParams } from "react-router";
import { useState } from "react";
import SessionForm from "../admin/components/SessionForm";
import { NEW_SESSION_CONFIG } from "../admin/components/sessionConfig";
import { useUserInfo } from "../admin/components/useUserInfo";
import { useNotify } from "../admin/components/useNotify";
import { useRetryableLazyLoadQuery } from "../components/useRetryableLazyLoadQuery";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
} from "../components/ui/Panel";
import type { SessionEnrollQuery } from "./__generated__/SessionEnrollQuery.graphql";
import type { SessionEnrollMutation } from "./__generated__/SessionEnrollMutation.graphql";

/**
 * Standalone page reached by scanning a kiosk's enrollment QR code (see EnrollApp.tsx
 * for why it lives outside the admin dashboard). The `?fp=` query param carries the
 * kiosk's public-key fingerprint. Confirms the pending enrollment is still live, then
 * creates a key-bound session via `enrollSession`.
 *
 * A device already enrolled as another kiosk can simply be enrolled again: the server
 * retires the old kiosk. This page says nothing about that kiosk — it may belong to a
 * location this admin has no access to, and the QR code is not a way to look it up.
 */
export default function SessionEnroll() {
  const [searchParams] = useSearchParams();
  const fingerprint = searchParams.get("fp");

  if (!fingerprint) {
    return (
      <Notice title="Missing device fingerprint">
        This enrollment link is missing its device fingerprint. Rescan the QR
        code shown on the kiosk.
      </Notice>
    );
  }

  return <SessionEnrollForm fingerprint={fingerprint} />;
}

function SessionEnrollForm({ fingerprint }: { fingerprint: string }) {
  const { notifyError } = useNotify();
  const { locations } = useUserInfo();
  const enabledLocations = locations
    .filter((loc) => loc.enabled)
    .sort((a, b) => a.name.localeCompare(b.name));
  const [enrolled, setEnrolled] = useState(false);

  const data = useRetryableLazyLoadQuery<SessionEnrollQuery>(
    graphql`
      query SessionEnrollQuery($fingerprint: String!) @throwOnFieldError {
        pendingEnrollmentKey(fingerprint: $fingerprint) {
          __typename
        }
      }
    `,
    { fingerprint },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<SessionEnrollMutation>(graphql`
      mutation SessionEnrollMutation(
        $name: String!
        $locationId: ID!
        $config: String
        $healthcheckUrl: String
        $keyFingerprint: String!
      ) {
        enrollSession(
          name: $name
          locationId: $locationId
          config: $config
          healthcheckUrl: $healthcheckUrl
          keyFingerprint: $keyFingerprint
        ) {
          id
        }
      }
    `);

  async function handleSubmit(formData: FormData) {
    const name = formData.get("name")?.toString() || "";
    const config = formData.get("config")?.toString() || "";
    const healthcheckUrl = formData.get("healthcheckUrl")?.toString() || "";
    const locationId = formData.get("locationId")?.toString() || "";

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: {
            name,
            locationId,
            config,
            healthcheckUrl,
            keyFingerprint: fingerprint,
          },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            const location = store.get(locationId);
            location?.invalidateRecord();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't enroll kiosk");
      return;
    }

    setEnrolled(true);
  }

  if (enrolled) {
    return (
      <Notice title="Kiosk enrolled">
        The kiosk screen will switch over automatically within a few seconds.
        You can close this page, or head to{" "}
        <Link to="/admin/sessions">the kiosks list</Link>.
      </Notice>
    );
  }

  if (data.pendingEnrollmentKey == null) {
    return (
      <Notice title="Enrollment request expired">
        This enrollment request has expired or the kiosk is no longer waiting.
        Check that the kiosk is still showing its QR code and rescan it.
      </Notice>
    );
  }

  if (enabledLocations.length === 0) {
    return (
      <Notice title="No locations available">
        No locations available to enroll this kiosk at. Please contact an
        administrator.
      </Notice>
    );
  }

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Enroll this kiosk</PanelTitle>
        <PanelIntro>
          Choose the location it belongs to and give it a name to describe the
          location or type of computer, then save — the kiosk will start working
          automatically, no code required.
        </PanelIntro>

        <SessionForm
          initialName=""
          initialConfig={NEW_SESSION_CONFIG}
          initialHealthcheckUrl=""
          locations={enabledLocations}
          isMutationInFlight={isMutationInFlight}
          onSubmit={handleSubmit}
        />
      </PanelBox>
    </Panel>
  );
}

function Notice({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <Panel>
      <PanelBox>
        <PanelTitle>{title}</PanelTitle>
        <PanelIntro>{children}</PanelIntro>
      </PanelBox>
    </Panel>
  );
}
