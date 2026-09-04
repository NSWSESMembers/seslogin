/* Icons for the two options on the home page.
 *
 * Inline rather than files under `public/image/` because these are static
 * chrome, not the data-driven category art: inlining lets the navy stroke be
 * `currentColor`, so the icons follow the `--color-navy` token into dark mode
 * and pick up the row's hover colour along with its text. Drawn on the same
 * 48-unit grid, 2.4 stroke and `#fb6c0d` accent as the category icons in
 * `assets/categories/`, so the two sets read as one family.
 */

type IconProps = { className?: string };

const strokeProps = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2.4,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

/** A check-in terminal: this computer, with a member signed in. */
export function KioskIcon({ className }: IconProps) {
  return (
    <svg
      viewBox="0 0 48 48"
      aria-hidden="true"
      className={className}
      {...strokeProps}
    >
      <rect x="5" y="7" width="38" height="26" rx="3" />
      <path d="M24 33 V39" />
      <path d="M15 40.5 H33" />
      <path
        d="M17 20 L21.5 24.5 L31 15"
        className="stroke-accent"
        strokeWidth={3}
      />
    </svg>
  );
}

/** A dashboard gauge: the administrator's view of unit activity. */
export function AdminIcon({ className }: IconProps) {
  return (
    <svg
      viewBox="0 0 48 48"
      aria-hidden="true"
      className={className}
      {...strokeProps}
    >
      <path d="M8.4 37 A18 18 0 1 1 39.6 37" />
      <path d="M11.2 20.6 L14.5 22.5 M24 13.2 V17 M36.8 20.6 L33.5 22.5" />
      <path d="M24 28 L30.1 16.5" className="stroke-accent" strokeWidth={3} />
      <circle cx="24" cy="28" r="2.1" className="fill-accent" stroke="none" />
    </svg>
  );
}
