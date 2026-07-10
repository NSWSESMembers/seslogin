import { forcePasskeyEnrollPrompt } from "../../lib/passkey";
import { Button } from "../../components/ui/Button";

export default function ShowPasskeyPromptButton() {
  return (
    <Button onClick={() => forcePasskeyEnrollPrompt()}>
      [DEV] Show Passkey Interstitial
    </Button>
  );
}
