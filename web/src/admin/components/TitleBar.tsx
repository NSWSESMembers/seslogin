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
    <div id="title_bar">
      <span>
        <a href="/admin" onClick={changeLocation} title="Click to change unit">
          {selectedLocation.name}
        </a>
      </span>
      <a href="/">
        <img src={logoWhite} alt="" />
      </a>
    </div>
  );
}
