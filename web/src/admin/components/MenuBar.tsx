import { useState } from "react";
import { matchPath, useLocation } from "react-router";
import MenuLink from "../../components/ui/MenuLink";
import { menuButtonClasses } from "../../components/ui/menuStyles";

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
      <button
        type="button"
        className="flex w-full cursor-pointer items-center justify-between border-0 bg-transparent px-5 py-2 text-lg text-white md:hidden"
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
      </div>
    </div>
  );
}
