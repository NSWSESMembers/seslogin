import { useSettingsDispatch } from "../../lib/settings";
import { Button } from "../../components/ui/Button";

export default function ClearLocationButton() {
  const settingsDispatch = useSettingsDispatch();

  return (
    <Button
      className="mt-4"
      onClick={() => settingsDispatch?.({ type: "clear_location" })}
    >
      [DEV] Clear Location
    </Button>
  );
}
