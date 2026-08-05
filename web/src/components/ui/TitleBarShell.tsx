import type { ReactNode } from "react";
import logoWhite from "../../assets/logo-white.svg";

/** Brand title bar chrome: the branded flex bar plus the home logo. Callers
 * provide the right-hand content (location link, breadcrumb, buttons) as children.
 *
 * `nonProdDb` paints the bar as a non-production warning. It's a prop rather than
 * a `useNonProdDb()` call because admin also renders this shell, and admin's
 * warning lives on its TopBar — only the kiosk, which has no TopBar, wants it here. */
export default function TitleBarShell({
  children,
  nonProdDb,
}: {
  children: ReactNode;
  nonProdDb?: boolean;
}) {
  return (
    <div
      className={
        nonProdDb
          ? "flex items-center gap-5 bg-danger-env p-2 pl-5 text-left font-title text-3xl text-white"
          : "flex items-center gap-5 bg-brand p-2 pl-5 text-left font-title text-3xl text-white"
      }
    >
      <a href="/">
        <img src={logoWhite} alt="" className="block" />
      </a>
      {children}
    </div>
  );
}
