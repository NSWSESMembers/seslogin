import { useEffect, useState } from "react";

const messages = [
  "Loading...",
  "Just a moment...",
  "Fetching data...",
  "Processing...",
  "Almost there...",
  "One second...",
];

export default function LoadingIndicator() {
  const [messageIndex, setMessageIndex] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => {
      setMessageIndex((prev) => (prev + 1) % messages.length);
    }, 800);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="flex flex-col items-center justify-center gap-4 px-6 py-12">
      <div className="size-12 animate-spin rounded-full border-4 border-neutral-200 border-t-menu motion-reduce:animate-none"></div>
      <div>{messages[messageIndex]}</div>
    </div>
  );
}
