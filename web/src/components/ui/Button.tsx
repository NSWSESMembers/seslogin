import { Link, type LinkProps } from "react-router";
import type { ButtonHTMLAttributes } from "react";
import { tw } from "../../lib/tw";
import {
  buttonVariants,
  buttonSizes,
  type ButtonVariant,
  type ButtonSize,
} from "./buttonStyles";

const base = tw`inline-block cursor-pointer align-middle no-underline`;

function classNames(
  variant: ButtonVariant,
  size: ButtonSize,
  className?: string,
) {
  return [base, buttonVariants[variant], buttonSizes[size], className]
    .filter(Boolean)
    .join(" ");
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
};

export function Button({
  variant = "primary",
  size = "normal",
  className,
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={classNames(variant, size, className)}
      {...props}
    />
  );
}

type ButtonLinkProps = LinkProps & {
  variant?: ButtonVariant;
  size?: ButtonSize;
};

export function ButtonLink({
  variant = "primary",
  size = "normal",
  className,
  ...props
}: ButtonLinkProps) {
  return <Link className={classNames(variant, size, className)} {...props} />;
}
