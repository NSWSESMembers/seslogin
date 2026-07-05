import type { ReactNode } from "react";
import logoWhite from "../../assets/logo-white.svg";

/** Brand title bar chrome: the branded flex bar plus the home logo. Callers
 * provide the right-hand content (location link, breadcrumb, buttons) as children. */
export default function TitleBarShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-5 bg-brand px-2 py-2 pl-5 text-left font-title text-[32px] text-white">
      <a href="/">
        <img src={logoWhite} alt="" className="block" />
      </a>
      {children}
    </div>
  );
}
