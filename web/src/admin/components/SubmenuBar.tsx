import { useMatch } from "react-router";
import MenuLink from "../../components/ui/MenuLink";

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
  const isSettingsSection = useMatch("/admin/settings/*");

  return (
    <>
      {isMembersSection && (
        <div className="bg-submenu px-4 py-0.75 text-left font-title lg:px-20">
          <MenuLink level="sub" to="/admin/members" end>
            List
          </MenuLink>
          <MenuLink level="sub" to="/admin/members/new" end>
            New
          </MenuLink>
        </div>
      )}

      {isActivitySection && (
        <div className="bg-submenu px-4 py-0.75 text-left font-title lg:px-20">
          <MenuLink level="sub" to="/admin/activity" end>
            Periods
          </MenuLink>
          <MenuLink level="sub" to="/admin/activity/new" end>
            New
          </MenuLink>
          <MenuLink level="sub" to="/admin/activity/current" end>
            Current
          </MenuLink>
          <MenuLink level="sub" to="/admin/activity/totals" end>
            Totals
          </MenuLink>
          <MenuLink level="sub" to="/admin/activity/breakdown" end>
            Breakdown
          </MenuLink>
          <MenuLink level="sub" to="/admin/activity/last-seen" end>
            Last Seen
          </MenuLink>
        </div>
      )}

      {isSessionsSection && (
        <div className="bg-submenu px-4 py-0.75 text-left font-title lg:px-20">
          <MenuLink level="sub" to="/admin/sessions" end>
            List
          </MenuLink>
          <MenuLink level="sub" to="/admin/sessions/new" end>
            New
          </MenuLink>
        </div>
      )}

      {isSuper && isLocationsSection && (
        <div className="bg-submenu px-4 py-0.75 text-left font-title lg:px-20">
          <MenuLink level="sub" to="/admin/locations" end>
            List
          </MenuLink>
          <MenuLink level="sub" to="/admin/locations/new" end>
            New
          </MenuLink>
        </div>
      )}

      {isSuper && isUsersSection && (
        <div className="bg-submenu px-4 py-0.75 text-left font-title lg:px-20">
          <MenuLink level="sub" to="/admin/users" end>
            List
          </MenuLink>
          <MenuLink level="sub" to="/admin/users/new" end>
            New
          </MenuLink>
        </div>
      )}

      {isSuper && isCategoriesSection && (
        <div className="bg-submenu px-4 py-0.75 text-left font-title lg:px-20">
          <MenuLink level="sub" to="/admin/categories" end>
            List Categories
          </MenuLink>
          <MenuLink level="sub" to="/admin/categories/new" end>
            New Category
          </MenuLink>
          <MenuLink level="sub" to="/admin/categories/nitc-groups" end>
            List NITC groups
          </MenuLink>
          <MenuLink level="sub" to="/admin/categories/nitc-groups/new" end>
            New NITC group
          </MenuLink>
        </div>
      )}

      {isSettingsSection && (
        <div className="bg-submenu px-4 py-0.75 text-left font-title lg:px-20">
          <MenuLink level="sub" to="/admin/settings" end>
            Passkeys
          </MenuLink>
          <MenuLink level="sub" to="/admin/settings/daily-email" end>
            Daily Email Summary
          </MenuLink>
        </div>
      )}
    </>
  );
}
