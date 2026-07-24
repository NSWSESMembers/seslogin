import { useKioskSession } from "./useKioskSession";
import TitleBarShell from "../../components/ui/TitleBarShell";

export default function ScanTitleBar(props: {
  onCancelSignOut?: () => void;
  signingOutName?: string;
}) {
  const session = useKioskSession();
  const locationName = session?.location.name ?? "Unknown location";
  const sessionName = session?.name ?? "Unknown kiosk";
  const title = props.signingOutName
    ? `${locationName} > ${sessionName} > ${props.signingOutName}`
    : `${locationName} > ${sessionName}`;

  return (
    <TitleBarShell>
      <span>{title}</span>
      {props.onCancelSignOut && (
        <button
          onClick={props.onCancelSignOut}
          className="ml-auto shrink-0 cursor-pointer rounded-lg border-2 border-white bg-transparent px-4 py-2 font-title text-[0.6em] leading-none text-white active:bg-white/20"
        >
          Cancel sign out
        </button>
      )}
    </TitleBarShell>
  );
}
