import ScanController from "../components/ScanController";
import ScanTitleBar from "../components/ScanTitleBar";
import ClientVersionLabel from "../../components/ClientVersionLabel";
import { useState } from "react";

export default function Scan() {
  const [cancelSignOut, setCancelSignOut] = useState<(() => void) | null>(null);
  const [signingOutName, setSigningOutName] = useState<string | null>(null);
  const [signingOutPersonId, setSigningOutPersonId] = useState<string | null>(
    null,
  );

  return (
    <div className="flex h-dvh flex-col">
      <ScanTitleBar
        onCancelSignOut={cancelSignOut ?? undefined}
        signingOutName={signingOutName ?? undefined}
        signingOutPersonId={signingOutPersonId ?? undefined}
      />
      <div className="relative flex-1 overflow-hidden">
        <ScanController
          onCancelSignOutChange={(fn) => setCancelSignOut(fn ? () => fn : null)}
          onSigningOutNameChange={setSigningOutName}
          onSigningOutPersonIdChange={setSigningOutPersonId}
        />
      </div>
      <div className="fixed right-2.5 bottom-1.5 text-[0.75em] text-neutral-400">
        <ClientVersionLabel noLink />
      </div>
    </div>
  );
}
