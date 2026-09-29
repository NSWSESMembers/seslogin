import TitleBarShell from "../../components/ui/TitleBarShell";
import { useSettingsDispatch } from "../../lib/settings";
import useSelectedLocation from "./useSelectedLocation";

export default function TitleBar() {
  const settingsDispatch = useSettingsDispatch();
  const selectedLocation = useSelectedLocation();

  const changeLocation = (e: React.MouseEvent<HTMLAnchorElement>) => {
    e.preventDefault();
    settingsDispatch?.({ type: "clear_location" });
  };

  return (
    <TitleBarShell>
      <a
        href="/admin"
        onClick={changeLocation}
        title="Click to change unit"
        className="text-white no-underline"
      >
        {selectedLocation.name}
      </a>
      {!selectedLocation.viewerCanEdit && (
        <span
          className="rounded-sm border border-current px-1.5 py-px align-middle font-sans text-xs tracking-wide text-white uppercase"
          title="You can view this location but not make changes"
        >
          Read only
        </span>
      )}
    </TitleBarShell>
  );
}
