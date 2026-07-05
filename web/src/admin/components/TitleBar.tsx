import logoWhite from "../../assets/logo-white.svg";
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
    <div className="flex items-center gap-5 bg-brand px-2 py-2 pl-5 text-left font-title text-[32px] text-white">
      <a href="/">
        <img src={logoWhite} alt="" className="block" />
      </a>
      <a
        href="/admin"
        onClick={changeLocation}
        title="Click to change unit"
        className="text-white no-underline"
      >
        {selectedLocation.name}
      </a>
    </div>
  );
}
