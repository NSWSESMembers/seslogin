import { useState } from "react";
import { matchPath, useLocation } from "react-router";
import MenuLink from "../../components/ui/MenuLink";
import {
  menuButtonClasses,
  menuLevelClasses,
} from "../../components/ui/menuStyles";
import { cn } from "../../lib/tw";

type NavItem = {
  to: string;
  label: string;
  end?: boolean;
  superOnly?: boolean;
};

const NAV_ITEMS: NavItem[] = [
  { to: "/admin", label: "Home", end: true },
  { to: "/admin/members", label: "Members" },
  { to: "/admin/activity", label: "Activity" },
  { to: "/admin/reports", label: "Reports" },
  { to: "/admin/sessions", label: "Kiosks" },
  { to: "/admin/audit", label: "Audit log" },
  { to: "/admin/settings", label: "Settings" },
  { to: "/admin/locations", label: "Locations", superOnly: true },
  { to: "/admin/users", label: "Users", superOnly: true },
  { to: "/admin/categories", label: "Categories", superOnly: true },
  { to: "/admin/api-tokens", label: "API Tokens", superOnly: true },
];

interface MenuBarProps {
  onLogout: () => void;
  isSuper: boolean;
}

export default function MenuBar({ onLogout, isSuper }: MenuBarProps) {
  const [open, setOpen] = useState(false);
  const location = useLocation();

  // collapse the mobile menu whenever navigation happens
  const [prevPathname, setPrevPathname] = useState(location.pathname);
  if (location.pathname !== prevPathname) {
    setPrevPathname(location.pathname);
    setOpen(false);
  }

  const items = NAV_ITEMS.filter((item) => !item.superOnly || isSuper);
  const currentItem = items.find((item) =>
    matchPath({ path: item.to, end: item.end ?? false }, location.pathname),
  );

  return (
    <div className="bg-menu font-title">
      {/* Phone: the guide link sits beside the menu toggle, so it's never
          tucked away inside the collapsed menu. */}
      <div className="flex items-center md:hidden">
        <button
          type="button"
          className="flex flex-1 cursor-pointer items-center justify-between border-0 bg-transparent px-5 py-2 text-lg text-white"
          aria-expanded={open}
          aria-controls="admin-menu-links"
          onClick={() => setOpen((v) => !v)}
        >
          {currentItem?.label ?? "Menu"}
          <svg
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            aria-hidden="true"
          >
            {open ? (
              <path d="M6 6l12 12M18 6L6 18" />
            ) : (
              <path d="M4 6h16M4 12h16M4 18h16" />
            )}
          </svg>
        </button>
        <GuideLink className="mr-3" />
      </div>
      <div
        id="admin-menu-links"
        className={`${open ? "flex" : "hidden"} flex-col gap-1 px-5 pb-3 md:flex md:flex-row md:flex-wrap md:items-center md:gap-1 md:py-0.75 lg:px-20`}
      >
        {items.map((item) => (
          <MenuLink key={item.to} to={item.to} end={item.end}>
            {item.label}
          </MenuLink>
        ))}
        <button className={menuButtonClasses} onClick={onLogout}>
          Logout
        </button>
        {/* Desktop: always present, pulled to the far right of the menu. */}
        <GuideLink className="max-md:hidden md:ml-auto" />
      </div>
    </div>
  );
}

/**
 * The user guide, kept visible on every screen size rather than in the menu list.
 * It's a public page outside the dashboard, so it opens in a new tab and the admin
 * keeps their place here.
 */
function GuideLink({ className }: { className?: string }) {
  return (
    <a
      href="/guide/admin"
      target="_blank"
      rel="noopener"
      className={cn(
        menuLevelClasses.main,
        "inline-flex items-center gap-1.5",
        className,
      )}
    >
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2z" />
        <path d="M4 19V5M8 7h7M8 11h7" />
      </svg>
      User guide
    </a>
  );
}
