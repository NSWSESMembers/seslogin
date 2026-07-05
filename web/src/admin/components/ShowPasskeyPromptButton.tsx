import { forcePasskeyEnrollPrompt } from "../../lib/passkey";
import { Button } from "../../components/ui/Button";

export default function ShowPasskeyPromptButton() {
  return (
    <Button className="mt-4" onClick={() => forcePasskeyEnrollPrompt()}>
      [DEV] Show Passkey Interstitial
    </Button>
  );
}
