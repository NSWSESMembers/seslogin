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
  const greenSeconds = 10 * 60;
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
