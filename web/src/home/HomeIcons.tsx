/* Icons for the two options on the home page.
 *
 * Inline rather than files under `assets/` because these are static chrome,
 * not the data-driven category art. Drawn on the same 48-unit grid, 2.4
 * stroke and `#fb6c0d` accent as the category icons in `assets/categories/`,
 * and sharing their color mechanism (`icon-stroke`/`icon-accent`/
 * `icon-accent-stroke`, see the comment in `app.css`) so the two sets read
 * as one family: navy follows `currentColor` (a `text-navy`/
 * `group-hover:text-...` class on the icon drives it, as Home.tsx already
 * does), and the orange accent follows the independent `--icon-accent`
 * custom property, so it can shift on hover/dark-mode without touching
 * `color`.
 */

type IconProps = { className?: string };

const strokeProps = {
  fill: "none",
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
      className={`icon-stroke ${className ?? ""}`}
      {...strokeProps}
    >
      <rect x="5" y="7" width="38" height="26" rx="3" />
      <path d="M24 33 V39" />
      <path d="M15 40.5 H33" />
      <path
        d="M17 20 L21.5 24.5 L31 15"
        className="icon-accent-stroke"
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
      className={`icon-stroke ${className ?? ""}`}
      {...strokeProps}
    >
      <path d="M8.4 37 A18 18 0 1 1 39.6 37" />
      <path d="M11.2 20.6 L14.5 22.5 M24 13.2 V17 M36.8 20.6 L33.5 22.5" />
      <path
        d="M24 28 L30.1 16.5"
        className="icon-accent-stroke"
        strokeWidth={3}
      />
      <circle cx="24" cy="28" r="2.1" className="icon-accent" stroke="none" />
    </svg>
  );
}
