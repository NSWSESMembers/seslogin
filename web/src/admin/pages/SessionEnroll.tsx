import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import { Link, useSearchParams } from "react-router";
import { useState } from "react";
import SessionForm from "../components/SessionForm";
import useSelectedLocation from "../components/useSelectedLocation";
import { useNotify } from "../components/useNotify";
import type { SessionEnrollQuery } from "./__generated__/SessionEnrollQuery.graphql";
import type { SessionEnrollMutation } from "./__generated__/SessionEnrollMutation.graphql";

/**
 * Admin page reached by scanning a kiosk's enrollment QR code. The `?fp=` query param
 * carries the kiosk's public-key fingerprint. Confirms the pending enrollment is still
 * live, then creates a key-bound session via `enrollSession`.
 */
export default function SessionEnroll() {
  const [searchParams] = useSearchParams();
  const fingerprint = searchParams.get("fp");

  if (!fingerprint) {
    return (
      <Notice>
        This enrollment link is missing its device fingerprint. Rescan the QR
        code shown on the kiosk.
      </Notice>
    );
  }

  return <SessionEnrollForm fingerprint={fingerprint} />;
}

function SessionEnrollForm({ fingerprint }: { fingerprint: string }) {
  const { notifyError } = useNotify();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const [enrolled, setEnrolled] = useState(false);

  const data = useLazyLoadQuery<SessionEnrollQuery>(
    graphql`
      query SessionEnrollQuery($fingerprint: String!) {
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
      <Notice>
        Kiosk enrolled. The kiosk screen will switch over automatically within a
        few seconds. <Link to="/admin/sessions">Back to kiosks</Link>
      </Notice>
    );
  }

  if (data.pendingEnrollmentKey == null) {
    return (
      <Notice>
        This enrollment request has expired or the kiosk is no longer waiting.
        Check that the kiosk is still showing its QR code and rescan it.
      </Notice>
    );
  }

  return (
    <>
      <p>
        You're enrolling a kiosk by its device key. Give it a name to describe
        the location or type of computer, then save — the kiosk will start
        working automatically, no code required.
      </p>

      <SessionForm
        initialName=""
        initialConfig="{}"
        initialHealthcheckUrl=""
        isMutationInFlight={isMutationInFlight}
        onSubmit={handleSubmit}
      />
    </>
  );
}

function Notice({ children }: { children: React.ReactNode }) {
  return <p className="mt-4">{children}</p>;
}
