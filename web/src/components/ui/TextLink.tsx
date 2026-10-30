import type { AnchorHTMLAttributes } from "react";
import { Link } from "react-router";
import { cn } from "../../lib/tw";

export const textLinkStyles =
  "text-navy underline underline-offset-2 hover:decoration-2";

type TextLinkProps = Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> &
  ({ to: string; href?: never } | { href: string; to?: never });

// A link inside running text, coloured and underlined so it stands out from the
// words around it. `to` is a route in this app (a router Link); `href` is
// anything else, such as a mailto: or another site.
export function TextLink({ to, href, className, ...props }: TextLinkProps) {
  const classes = cn(textLinkStyles, className);
  return to !== undefined ? (
    <Link to={to} className={classes} {...props} />
  ) : (
    <a href={href} className={classes} {...props} />
  );
}
