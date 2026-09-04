import { useState } from "react";
import { matchPath, useLocation, useMatch } from "react-router";
import MenuLink from "../../components/ui/MenuLink";

type SubItem = { to: string; label: string };

function Submenu({ items }: { items: SubItem[] }) {
  const [open, setOpen] = useState(false);
  const location = useLocation();
  const current = items.find((item) =>
    matchPath({ path: item.to, end: true }, location.pathname),
  );

  // collapse the mobile submenu whenever navigation happens
  const [prevPathname, setPrevPathname] = useState(location.pathname);
  if (location.pathname !== prevPathname) {
    setPrevPathname(location.pathname);
    setOpen(false);
  }

  return (
    <div className="bg-submenu font-title">
      <button
        type="button"
        className="flex w-full cursor-pointer items-center justify-between border-0 bg-transparent px-5 py-2 text-sm text-white md:hidden"
        aria-expanded={open}
        aria-controls="admin-submenu-links"
        onClick={() => setOpen((v) => !v)}
      >
        {current?.label ?? "Select a page"}
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
          className={open ? "rotate-180" : ""}
        >
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>
      <div
        id="admin-submenu-links"
        className={`${open ? "flex" : "hidden"} flex-col gap-1 px-5 pb-3 md:flex md:flex-row md:flex-wrap md:items-center md:gap-1 md:px-5 md:py-0.75 lg:px-20`}
      >
        {items.map((item) => (
          <MenuLink key={item.to} level="sub" to={item.to} end>
            {item.label}
          </MenuLink>
        ))}
      </div>
    </div>
  );
}

interface SubmenuBarProps {
  isSuper: boolean;
}

export default function SubmenuBar({ isSuper }: SubmenuBarProps) {
  const isMembersSection = useMatch("/admin/members/*");
  const isActivitySection = useMatch("/admin/activity/*");
  const isSessionsSection = useMatch("/admin/sessions/*");
  const isLocationsSection = useMatch("/admin/locations/*");
  const isUsersSection = useMatch("/admin/users/*");
  const isCategoriesSection = useMatch("/admin/categories/*");
  const isApiTokensSection = useMatch("/admin/api-tokens/*");
  const isSettingsSection = useMatch("/admin/settings/*");

  if (isMembersSection) {
    return (
      <Submenu
        items={[
          { to: "/admin/members", label: "List" },
          { to: "/admin/members/new", label: "New" },
        ]}
      />
    );
  }

  if (isActivitySection) {
    return (
      <Submenu
        items={[
          { to: "/admin/activity", label: "Previous Periods" },
          { to: "/admin/activity/new", label: "New Period" },
          { to: "/admin/activity/current", label: "Incomplete Periods" },
          { to: "/admin/activity/totals", label: "Totals" },
          { to: "/admin/activity/breakdown", label: "Breakdown" },
          { to: "/admin/activity/daily-breakdown", label: "Daily Breakdown" },
          { to: "/admin/activity/heatmap", label: "Heatmap" },
          { to: "/admin/activity/last-seen", label: "Last Seen" },
        ]}
      />
    );
  }

  if (isSessionsSection) {
    return (
      <Submenu
        items={[
          { to: "/admin/sessions", label: "List" },
          { to: "/admin/sessions/new", label: "New" },
        ]}
      />
    );
  }

  if (isSuper && isLocationsSection) {
    return (
      <Submenu
        items={[
          { to: "/admin/locations", label: "List" },
          { to: "/admin/locations/new", label: "New" },
        ]}
      />
    );
  }

  if (isSuper && isUsersSection) {
    return (
      <Submenu
        items={[
          { to: "/admin/users", label: "List" },
          { to: "/admin/users/new", label: "New" },
        ]}
      />
    );
  }

  if (isSuper && isCategoriesSection) {
    return (
      <Submenu
        items={[
          { to: "/admin/categories", label: "List Categories" },
          { to: "/admin/categories/new", label: "New Category" },
          { to: "/admin/categories/nitc-groups", label: "List NITC groups" },
          {
            to: "/admin/categories/nitc-groups/new",
            label: "New NITC group",
          },
        ]}
      />
    );
  }

  if (isSuper && isApiTokensSection) {
    return (
      <Submenu
        items={[
          { to: "/admin/api-tokens", label: "List" },
          { to: "/admin/api-tokens/new", label: "New" },
        ]}
      />
    );
  }

  if (isSettingsSection) {
    return (
      <Submenu
        items={[
          { to: "/admin/settings", label: "Passkeys" },
          { to: "/admin/settings/daily-email", label: "Daily Email Summary" },
          { to: "/admin/settings/activity-display", label: "Activity Display" },
        ]}
      />
    );
  }

  return null;
}
