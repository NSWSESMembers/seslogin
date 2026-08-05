import type { ReactNode } from "react";
import logoWhite from "../../assets/logo-white.svg";

/** Brand title bar chrome: the branded flex bar plus the home logo. Callers
 * provide the right-hand content (location link, breadcrumb, buttons) as children.
 *
 * The logo links home by default. Pass `onLogoClick` to make it a button instead —
 * kiosks use this so tapping the logo opens the status panel rather than navigating
 * the device out of the kiosk app. */
export default function TitleBarShell({
  children,
  onLogoClick,
}: {
  children: ReactNode;
  onLogoClick?: () => void;
}) {
  const logo = <img src={logoWhite} alt="" className="block" />;

  return (
    <div className="flex items-center gap-5 bg-brand p-2 pl-5 text-left font-title text-3xl text-white">
      {onLogoClick ? (
        <button
          type="button"
          onClick={onLogoClick}
          aria-label="Kiosk status"
          className="cursor-pointer border-0 bg-transparent p-0"
        >
          {logo}
        </button>
      ) : (
        <a href="/">{logo}</a>
      )}
      {children}
    </div>
  );
}
