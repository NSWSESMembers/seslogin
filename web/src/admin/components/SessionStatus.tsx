import { useState } from "react";
import bulletGreen from "../../assets/bullet-green.svg";
import bulletOrange from "../../assets/bullet-orange.svg";
import bulletRed from "../../assets/bullet-red.svg";
import bulletGray from "../../assets/bullet-gray.svg";

export default function SessionStatus({
  lastContact,
}: {
  lastContact: number | null | undefined;
}) {
  const [nowSeconds] = useState(() => Math.floor(Date.now() / 1000));
  // On prod, a kiosk with realtime session pushes checks in every 15 min ±10% (see
  // LIVE_SESSION_REFRESH_INTERVAL_MS); matches the server's ONLINE_SESSION_SECONDS.
  const greenSeconds = 20 * 60;
  const orangeSeconds = 24 * 60 * 60;

  const status =
    lastContact == null
      ? "grey"
      : nowSeconds - lastContact <= greenSeconds
        ? "green"
        : nowSeconds - lastContact < orangeSeconds
          ? "orange"
          : "red";

  const iconSrc =
    status === "green"
      ? bulletGreen
      : status === "orange"
        ? bulletOrange
        : status === "red"
          ? bulletRed
          : bulletGray;

  return (
    <img
      src={iconSrc}
      alt=""
      width={16}
      height={16}
      className="inline-block max-w-none align-middle"
    />
  );
}
