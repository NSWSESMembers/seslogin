import { NavLink, type NavLinkProps } from "react-router";
import { cn } from "../../lib/tw";
import { menuLevelClasses, type MenuLevel } from "./menuStyles";

export default function MenuLink({
  level = "main",
  className,
  ...props
}: NavLinkProps & { level?: MenuLevel }) {
  return (
    <NavLink
      className={(renderProps) =>
        cn(
          menuLevelClasses[level],
          typeof className === "function" ? className(renderProps) : className,
        )
      }
      {...props}
    />
  );
}
