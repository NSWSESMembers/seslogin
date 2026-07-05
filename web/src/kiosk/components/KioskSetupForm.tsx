import { graphql, useMutation } from "react-relay";
import type { KioskSetupFormMutation } from "./__generated__/KioskSetupFormMutation.graphql";
import useKioskEnvironment from "./useKioskEnvironment";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
} from "../../components/ui/Panel";
import { Button } from "../../components/ui/Button";
import TextInput from "../../components/ui/TextInput";

const kioskSetupFormMutation = graphql`
  mutation KioskSetupFormMutation($code: String!) {
    authSession(code: $code)
  }
`;

export default function KioskSetupForm() {
  const { setToken } = useKioskEnvironment();
  const [commitMutation, isMutationInFlight] =
    useMutation<KioskSetupFormMutation>(kioskSetupFormMutation);

  const onSubmit = async (data: FormData) => {
    const code = data.get("code")?.toString() || "";

    await new Promise<void>((resolve) =>
      commitMutation({
        variables: { code },
        onCompleted: (res: { authSession?: string | null }) => {
          if (res.authSession) {
            setToken(res.authSession);
          } else {
            alert("Login failed");
          }
          resolve();
        },
        onError: (error: Error) => {
          console.log(error);
          alert("Login failed: " + error.message);
          resolve();
        },
      }),
    );
  };

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Setup this kiosk</PanelTitle>
        <PanelIntro>
          To setup the kiosk system on this computer you need to have someone
          with administrator access generate a kiosk setup code via the
          administrator dashboard. Enter the 6-digit kiosk setup code below to
          register this computer with the system.
        </PanelIntro>

        <form action={onSubmit}>
          <TextInput
            className="mb-3 box-border block min-w-65 p-3 text-xl"
            type="text"
            name="code"
            inputMode="numeric"
            maxLength={6}
            placeholder="Enter 6 digit code"
            aria-label="Kiosk code"
          />
          <Button type="submit" size="panel" disabled={isMutationInFlight}>
            {isMutationInFlight ? "Submitting..." : "Continue"}
          </Button>
        </form>
      </PanelBox>
    </Panel>
  );
}
