import { useKioskSession } from "./useKioskSession";
import TitleBarShell from "../../components/ui/TitleBarShell";
import { useNonProdDb } from "../../lib/environmentInfo";

export default function ScanTitleBar(props: {
  onCancelSignOut?: () => void;
  signingOutName?: string;
  onLogoClick?: () => void;
}) {
  const session = useKioskSession();
  const nonProdDb = useNonProdDb();
  const locationName = session?.location.name ?? "Unknown location";
  const sessionName = session?.name ?? "Unknown kiosk";
  const title = props.signingOutName
    ? `${locationName} > ${sessionName} > ${props.signingOutName}`
    : `${locationName} > ${sessionName}`;

  // Sticky with a solid background, padded below the iOS status bar, and above
  // the kiosk's dialogs (`z-10`) so nothing ever covers the top of the screen.
  // In a home-screen app iOS 26+ fills the status bar with this sticky bar's
  // colour (it ignores `theme-color`) and stops blurring the content beneath it.
  return (
    <div
      className={
        nonProdDb
          ? "sticky top-0 z-20 bg-danger-env pt-[env(safe-area-inset-top)]"
          : "sticky top-0 z-20 bg-brand pt-[env(safe-area-inset-top)]"
      }
    >
      <TitleBarShell nonProdDb={nonProdDb} onLogoClick={props.onLogoClick}>
        <span>{title}</span>
        {props.onCancelSignOut && (
          <button
            onClick={props.onCancelSignOut}
            className="ml-auto shrink-0 cursor-pointer rounded-lg border-2 border-white bg-transparent px-4 py-2 font-title text-lg leading-none text-white active:bg-white/20"
          >
            Cancel sign out
          </button>
        )}
      </TitleBarShell>
    </div>
  );
}
