import ClearLocationButton from "../components/ClearLocationButton";
import DevOnly from "../components/DevOnly";

export default function AdminHome() {
  return (
    <div>
      <p>
        Welcome the unit dashboard. Use this section of the website to
        administer your unit and monitor it's activity.
      </p>

      <DevOnly>
        <ClearLocationButton />
      </DevOnly>
    </div>
  );
}
