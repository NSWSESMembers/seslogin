import { NavLink, type NavLinkProps } from "react-router";
import { menuLevelClasses, type MenuLevel } from "./menuStyles";

export default function MenuLink({
  level = "main",
  className,
  ...props
}: NavLinkProps & { level?: MenuLevel }) {
  return (
    <NavLink
      className={[menuLevelClasses[level], className].filter(Boolean).join(" ")}
      {...props}
    />
  );
}
