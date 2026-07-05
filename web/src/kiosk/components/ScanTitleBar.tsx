import { useKioskSession } from "./useKioskSession";
import logoWhite from "../../assets/logo-white.svg";

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
    <div className="relative bg-brand px-3.75 py-3.75 pl-5 text-left font-title text-[32px] text-white">
      <span className="absolute top-5 left-20">{title}</span>
      <a href="/">
        <img src={logoWhite} alt="" className="align-middle" />
      </a>
      {props.onCancelSignOut && (
        <button
          onClick={props.onCancelSignOut}
          className="absolute top-1/2 right-3.75 -translate-y-1/2 cursor-pointer rounded-lg border-2 border-white bg-transparent px-4 py-2.5 font-title text-[0.6em] text-white active:bg-white/20"
        >
          Cancel sign out
        </button>
      )}
    </div>
  );
}
