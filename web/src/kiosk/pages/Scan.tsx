import ScanController from "../components/ScanController";
import ScanTitleBar from "../components/ScanTitleBar";
import KioskStatusDialog from "../components/KioskStatusDialog";
import ClientVersionLabel from "../../components/ClientVersionLabel";
import { useRef, useState } from "react";
import CategoryScaleProbe from "../components/CategoryScaleProbe";
import { useCategoryScale } from "../useCategoryScale";

export default function Scan() {
  const [cancelSignOut, setCancelSignOut] = useState<(() => void) | null>(null);
  const [signingOutName, setSigningOutName] = useState<string | null>(null);
  const [statusOpen, setStatusOpen] = useState(false);
  const hostRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLDivElement>(null);
  const categoryScale = useCategoryScale(hostRef, probeRef);

  return (
    <div className="flex h-dvh flex-col">
      <ScanTitleBar
        onCancelSignOut={cancelSignOut ?? undefined}
        signingOutName={signingOutName ?? undefined}
        onLogoClick={() => setStatusOpen(true)}
      />
      <div
        ref={hostRef}
        // Inherited by the category and quick pick buttons; see categoryScale.ts.
        style={{ "--cat-scale": categoryScale } as React.CSSProperties}
        className="relative flex-1 overflow-hidden"
      >
        <CategoryScaleProbe ref={probeRef} />
        <ScanController
          onCancelSignOutChange={(fn) => setCancelSignOut(fn ? () => fn : null)}
          onSigningOutNameChange={setSigningOutName}
        />
      </div>
      <div className="fixed right-2.5 bottom-1.5 text-xs text-neutral-400">
        <ClientVersionLabel noLink />
      </div>
      {statusOpen && (
        <KioskStatusDialog
          onClose={() => setStatusOpen(false)}
          categoryScale={categoryScale}
        />
      )}
    </div>
  );
}
