import { NavLink, useMatch } from "react-router";

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
  const isReportsSection = useMatch("/admin/reports/*");
  const isSettingsSection = useMatch("/admin/settings/*");

  return (
    <>
      {isMembersSection && (
        <div id="submenu_bar">
          <NavLink to="/admin/members" end>
            List
          </NavLink>
          <NavLink to="/admin/members/new" end>
            New
          </NavLink>
        </div>
      )}

      {isActivitySection && (
        <div id="submenu_bar">
          <NavLink to="/admin/activity" end>
            Periods
          </NavLink>
          <NavLink to="/admin/activity/new" end>
            New
          </NavLink>
          <NavLink to="/admin/activity/current" end>
            Current
          </NavLink>
          <NavLink to="/admin/activity/totals" end>
            Totals
          </NavLink>
          <NavLink to="/admin/activity/breakdown" end>
            Breakdown
          </NavLink>
          <NavLink to="/admin/activity/last-seen" end>
            Last Seen
          </NavLink>
        </div>
      )}

      {isSessionsSection && (
        <div id="submenu_bar">
          <NavLink to="/admin/sessions" end>
            List
          </NavLink>
          <NavLink to="/admin/sessions/new" end>
            New
          </NavLink>
        </div>
      )}

      {isSuper && isLocationsSection && (
        <div id="submenu_bar">
          <NavLink to="/admin/locations" end>
            List
          </NavLink>
          <NavLink to="/admin/locations/new" end>
            New
          </NavLink>
        </div>
      )}

      {isSuper && isUsersSection && (
        <div id="submenu_bar">
          <NavLink to="/admin/users" end>
            List
          </NavLink>
          <NavLink to="/admin/users/new" end>
            New
          </NavLink>
        </div>
      )}

      {isSuper && isCategoriesSection && (
        <div id="submenu_bar">
          <NavLink to="/admin/categories" end>
            List Categories
          </NavLink>
          <NavLink to="/admin/categories/new" end>
            New Category
          </NavLink>
          <NavLink to="/admin/categories/nitc-groups" end>
            List NITC groups
          </NavLink>
          <NavLink to="/admin/categories/nitc-groups/new" end>
            New NITC group
          </NavLink>
        </div>
      )}

      {isReportsSection && (
        <div id="submenu_bar">
          <NavLink to="/admin/reports" end>
            Activity Export
          </NavLink>
          <NavLink to="/admin/reports/badges" end>
            Badge Report
          </NavLink>
        </div>
      )}

      {isSettingsSection && (
        <div id="submenu_bar">
          <NavLink to="/admin/settings" end>
            Passkeys
          </NavLink>
          <NavLink to="/admin/settings/daily-email" end>
            Daily Email Summary
          </NavLink>
        </div>
      )}
    </>
  );
}
