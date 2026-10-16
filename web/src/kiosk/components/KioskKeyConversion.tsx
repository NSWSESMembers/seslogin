import { useEffect } from "react";
import { graphql, useRelayEnvironment } from "react-relay";
import { commitMutation } from "relay-runtime";
import type { KioskKeyConversionMutation } from "./__generated__/KioskKeyConversionMutation.graphql";
import { useKioskSession } from "./useKioskSession";
import { startKeyConversion } from "../lib/keyConversionRunner";

const convertMutation = graphql`
  mutation KioskKeyConversionMutation($publicKey: String!, $proof: String!) {
    convertSessionToKey(publicKey: $publicKey, proof: $proof) {
      id
    }
  }
`;

/**
 * Mounted only while a kiosk runs on a legacy setup-code JWT. Once the kiosk's
 * conversion time comes (`keyConversion.ts`), it attaches the kiosk's key to its own
 * session and reloads the page into key mode. Renders nothing.
 */
export function KioskKeyConversion({ profile }: { profile: string }) {
  const environment = useRelayEnvironment();
  const sessionId = useKioskSession()?.id;

  useEffect(() => {
    if (!sessionId) {
      return;
    }
    return startKeyConversion({
      profile,
      sessionId,
      commit: (publicKey, proof) =>
        new Promise((resolve, reject) => {
          commitMutation<KioskKeyConversionMutation>(environment, {
            mutation: convertMutation,
            variables: { publicKey, proof },
            onCompleted: (_, errors) => {
              if (errors && errors.length > 0) {
                reject(new Error(errors.map((e) => e.message).join("; ")));
              } else {
                resolve();
              }
            },
            onError: reject,
          });
        }),
    });
  }, [environment, profile, sessionId]);

  return null;
}
