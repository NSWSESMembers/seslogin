import { forcePasskeyEnrollPrompt } from "../../lib/passkey";

export default function ShowPasskeyPromptButton() {
  return (
    <button
      onClick={() => forcePasskeyEnrollPrompt()}
      style={{ marginTop: "1rem", padding: "0.5rem 1rem" }}
    >
      [DEV] Show Passkey Interstitial
    </button>
  );
}
