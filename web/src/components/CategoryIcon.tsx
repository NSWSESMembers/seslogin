import { categoryIconMarkup } from "../assets/categories";

/**
 * Renders a category's icon inline (not `<img>`), so its `icon-stroke`/`icon-fill`
 * (navy, via `currentColor`) and `icon-accent`/`icon-accent-stroke` (orange, via
 * the `--icon-accent` custom property) classes can pick up theme, hover, and
 * active state from an ancestor - see the icon color comment in `app.css`.
 *
 * Defaults to the icons' original navy/orange so existing call sites are
 * unaffected; override via `className` (`text-...` for the stroke/fill channel)
 * or an `[--icon-accent:...]` arbitrary-property utility for the accent channel.
 */
export function CategoryIcon({
  icon,
  className,
  style,
}: {
  icon: string;
  className?: string;
  style?: React.CSSProperties;
}) {
  const markup = categoryIconMarkup[icon];
  if (!markup) {
    return null;
  }

  return (
    <span
      aria-hidden="true"
      data-icon="category"
      style={style}
      className={`inline-block text-navy [&>svg]:size-full ${className ?? ""}`}
      dangerouslySetInnerHTML={{ __html: markup }}
    />
  );
}
