import { useNonProdDb } from "../../lib/environmentInfo";

interface TopBarProps {
  username: string;
}

export default function TopBar({ username }: TopBarProps) {
  // Shared by admin and the public home page, so both get the warning colour
  // from this one hook rather than each having to pass it down.
  const nonProdDb = useNonProdDb();

  return (
    <div
      className={
        nonProdDb
          ? "relative flex items-center justify-between bg-danger-env px-2 py-0.5 text-sm font-bold text-white"
          : "relative flex items-center justify-between bg-black px-2 py-0.5 text-sm font-bold text-white"
      }
    >
      <a href="/" className="text-white no-underline hover:underline">
        SES Activity
      </a>
      {nonProdDb && (
        // Absolutely positioned so it centres on the bar itself rather than on
        // whatever space is left between the link and a variable-length username.
        // `pointer-events-none` keeps it from swallowing clicks on the link.
        <span className="pointer-events-none absolute left-1/2 -translate-x-1/2 whitespace-nowrap">
          NON-PRODUCTION DATABASE
        </span>
      )}
      <span>{username}</span>
    </div>
  );
}
