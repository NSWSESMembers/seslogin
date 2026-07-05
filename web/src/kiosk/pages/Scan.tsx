import ScanController from "../components/ScanController";
import ScanTitleBar from "../components/ScanTitleBar";
import ClientVersionLabel from "../../components/ClientVersionLabel";
import { useState } from "react";

export default function Scan() {
  const [cancelSignOut, setCancelSignOut] = useState<(() => void) | null>(null);
  const [signingOutName, setSigningOutName] = useState<string | null>(null);

  return (
    <div>
      <ScanTitleBar
        onCancelSignOut={cancelSignOut ?? undefined}
        signingOutName={signingOutName ?? undefined}
      />
      <div className="absolute inset-x-0 top-24.75 bottom-0 m-0 overflow-hidden">
        <ScanController
          onCancelSignOutChange={(fn) => setCancelSignOut(fn ? () => fn : null)}
          onSigningOutNameChange={setSigningOutName}
        />
      </div>
      <div className="fixed right-2.5 bottom-1.5 text-[0.75em] text-neutral-400">
        <ClientVersionLabel noLink />
      </div>
    </div>
  );
}
