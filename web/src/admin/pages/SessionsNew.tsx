import { graphql } from "relay-runtime";
import { useState } from "react";
import { useMutation } from "react-relay";
import SessionForm from "../components/SessionForm";
import SessionCreatedInterstitial from "../components/SessionCreatedInterstitial";
import useSelectedLocation from "../components/useSelectedLocation";
import type { SessionsNewMutation } from "./__generated__/SessionsNewMutation.graphql";
import { useNotify } from "../components/useNotify";

export default function SessionsNew() {
  const { notifyError } = useNotify();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const [createdCode, setCreatedCode] = useState<string | null>(null);
  const [commitMutation, isMutationInFlight] = useMutation<SessionsNewMutation>(
    graphql`
      mutation SessionsNewMutation(
        $name: String!
        $locationId: ID!
        $config: String
        $healthcheckUrl: String
      ) {
        createSession(
          name: $name
          locationId: $locationId
          config: $config
          healthcheckUrl: $healthcheckUrl
        ) {
          id
          code
        }
      }
    `,
  );

  async function handleSubmit(formData: FormData) {
    const name = formData.get("name")?.toString() || "";
    const config = formData.get("config")?.toString() || "";
    const healthcheckUrl = formData.get("healthcheckUrl")?.toString() || "";
    let response: SessionsNewMutation["response"];
    try {
      response = await new Promise<SessionsNewMutation["response"]>(
        (resolve, reject) => {
          commitMutation({
            variables: { name, locationId, config, healthcheckUrl },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              const location = store.get(locationId);
              location?.invalidateRecord();
            },
          });
        },
      );
    } catch (err) {
      notifyError(err, "Couldn't create kiosk");
      return;
    }

    setCreatedCode(response.createSession.code ?? "");
  }

  if (createdCode != null) {
    return <SessionCreatedInterstitial code={createdCode} />;
  }

  return (
    <>
      <p>
        Please enter a name to describe the location or type of computer that
        you are setting up to be a kiosk. This will help you identify it later
        if you set up more than one.
      </p>
      {/* {error && <p className="font-bold text-red-600">Error: {error.message}</p>} */}

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
