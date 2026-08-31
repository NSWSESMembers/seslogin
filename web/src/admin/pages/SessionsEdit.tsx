import { graphql, useMutation } from "react-relay";
import { useNavigate, useParams } from "react-router";
import SessionForm from "../components/SessionForm";
import SessionClientInfo from "../components/SessionClientInfo";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { SessionsEditMutation } from "./__generated__/SessionsEditMutation.graphql";
import type { SessionsEditQuery } from "./__generated__/SessionsEditQuery.graphql";
import { useNotify } from "../components/useNotify";

export default function SessionsEdit() {
  const navigate = useNavigate();
  const params = useParams();
  const { notifyError, notifySuccess } = useNotify();
  const id = params.sessionId!;

  // The `clientInfo` selection is handed to SessionClientInfo whole rather than read
  // field by field, so the lint rule can't see the usage. Same disable as in
  // kiosk/components/KioskTokenSessionFetcher.tsx.
  /* eslint-disable relay/unused-fields */
  const data = useRetryableLazyLoadQuery<SessionsEditQuery>(
    graphql`
      query SessionsEditQuery($id: ID!) @throwOnFieldError {
        session(id: $id) {
          name
          config
          healthcheckUrl
          clientInfo {
            env
            origin
            apiUrl
            profile
            userAgent
            screen
            displayMode
            timezone
            clockSkewSecs
            uptimeSecs
            pendingVersion
            contactFailures
            updatedAt
          }
        }
      }
    `,
    { id },
  );
  /* eslint-enable relay/unused-fields */

  const [commitMutation, isMutationInFlight] =
    useMutation<SessionsEditMutation>(graphql`
      mutation SessionsEditMutation(
        $id: ID!
        $name: String!
        $config: String
        $healthcheckUrl: String
      ) {
        updateSession(
          id: $id
          name: $name
          config: $config
          healthcheckUrl: $healthcheckUrl
        ) {
          __typename
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
          variables: { id, name, config, healthcheckUrl },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't save kiosk");
      return;
    }

    notifySuccess("Kiosk saved");
    navigate("/admin/sessions");
  }

  const session = data.session;
  const configString = JSON.stringify(session.config ?? {}, null, 2);

  return (
    <>
      <p>
        Edit this kiosk's configuration, then click Save. The configuration
        update will be automatically applied within 5 minutes. Refresh the
        kiosk's webpage to reload the configuration immediately.
      </p>

      <SessionForm
        initialName={session.name}
        initialConfig={configString}
        initialHealthcheckUrl={session.healthcheckUrl ?? ""}
        isMutationInFlight={isMutationInFlight}
        onSubmit={handleSubmit}
      />

      <h2 className="mt-8 mb-2 text-lg font-semibold">Kiosk diagnostics</h2>
      <p className="mb-4 text-ink-muted">
        What this kiosk last reported about itself, refreshed on its regular
        check-in. Read-only.
      </p>
      <SessionClientInfo clientInfo={session.clientInfo} />
    </>
  );
}
