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
    <div className="relative bg-brand px-3.75 py-3.75 pl-5 text-left font-title text-[32px] text-white">
      <span className="absolute top-5 left-20">
        <a
          href="/admin"
          onClick={changeLocation}
          title="Click to change unit"
          className="text-white no-underline"
        >
          {selectedLocation.name}
        </a>
      </span>
      <a href="/">
        <img src={logoWhite} alt="" className="align-middle" />
      </a>
    </div>
  );
}
