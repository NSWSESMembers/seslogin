import MenuLink from "../../components/ui/MenuLink";
import { menuButtonClasses } from "../../components/ui/menuStyles";

interface MenuBarProps {
  onLogout: () => void;
  isSuper: boolean;
}

export default function MenuBar({ onLogout, isSuper }: MenuBarProps) {
  return (
    <div className="bg-menu px-4 py-0.75 text-left font-title lg:px-20">
      <MenuLink to="/admin" end>
        Home
      </MenuLink>
      <MenuLink to="/admin/members">Members</MenuLink>
      <MenuLink to="/admin/activity">Activity</MenuLink>
      <MenuLink to="/admin/reports">Reports</MenuLink>
      <MenuLink to="/admin/sessions">Kiosks</MenuLink>
      <MenuLink to="/admin/settings">Settings</MenuLink>
      {isSuper && <MenuLink to="/admin/locations">Locations</MenuLink>}
      {isSuper && <MenuLink to="/admin/users">Users</MenuLink>}
      {isSuper && <MenuLink to="/admin/categories">Categories</MenuLink>}
      <button className={menuButtonClasses} onClick={onLogout}>
        Logout
      </button>
    </div>
  );
}
