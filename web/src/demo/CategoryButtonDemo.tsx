import { useNavigate, useParams } from "react-router";
import { CategoryButton } from "../kiosk/components/ScanScreenCategories";
import {
  categories,
  categoryIconSrc,
  findLeafCategory,
} from "../lib/categories";

// Standalone preview of the kiosk category button (CategoryButton, from
// ScanScreenCategories) and its icon, for a given category id — every size,
// background and theme combination in one place without walking a scan
// through to the category screen. Route: /demo/category/:id?

const DEFAULT_ID = "RX2bfpU6ppvV"; // Training > AIIMS

const ICON_SIZES = [32, 48, 70, 120] as const;

const ICON_BACKGROUNDS: {
  label: string;
  className: string;
  theme?: "light" | "dark";
}[] = [
  { label: "White", className: "bg-white" },
  { label: "Black", className: "bg-black" },
  {
    label: "Dark surface (data-theme=dark)",
    className: "bg-surface",
    theme: "dark",
  },
  { label: "Brand accent", className: "bg-accent" },
];

function ThemedButtonTile(props: {
  theme: "light" | "dark";
  small: boolean;
  id: string;
  name: string;
  icon: string;
}) {
  const { theme, small, id, name, icon } = props;
  return (
    <div
      data-theme={theme}
      className="flex flex-col items-center gap-2 rounded-lg bg-surface p-4"
    >
      <span className="text-xs tracking-wide text-ink-muted uppercase">
        {theme} · {small ? "small" : "large"}
      </span>
      <ul className="m-0 p-0">
        <CategoryButton
          id={id}
          name={name}
          icon={icon}
          onSelect={() => {}}
          small={small}
        />
      </ul>
      <span className="text-xs text-ink-muted">
        click and hold to preview the pressed state
      </span>
    </div>
  );
}

export default function CategoryButtonDemo() {
  const navigate = useNavigate();
  const { id: routeId } = useParams<{ id?: string }>();
  const id = routeId || DEFAULT_ID;

  const match = findLeafCategory(id);

  return (
    <div className="mx-auto flex min-h-screen max-w-350 flex-col gap-8 bg-surface p-8 text-ink">
      <div>
        <h1 className="m-0 text-3xl font-bold">Category button demo</h1>
        <p className="text-ink-muted">
          Renders the real kiosk <code>CategoryButton</code> (from{" "}
          <code>ScanScreenCategories</code>) for a category id, at every size,
          theme and state it supports on the scan screen, plus the icon alone on
          a few backgrounds. The icon's own stroke color is baked into the SVG
          and does not adapt to dark mode — that's expected, not a bug in this
          demo.
        </p>
      </div>

      <label className="flex flex-wrap items-center gap-2">
        <span className="font-semibold">Category:</span>
        <select
          className="rounded-sm border-2 border-line-strong bg-surface-raised px-2 py-1 text-ink"
          value={match ? id : ""}
          onChange={(e) => navigate(`/demo/category/${e.target.value}`)}
        >
          {!match && (
            <option value="" disabled>
              {id} (not found in the kiosk icon set — pick one below)
            </option>
          )}
          {categories.map((top) => (
            <optgroup key={top.id} label={top.name}>
              {(top.subcategories || []).map((sub) => (
                <option key={sub.id} value={sub.id}>
                  {sub.name}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
        <span className="font-mono text-sm text-ink-muted">id={id}</span>
      </label>

      {!match ? (
        <p className="text-danger-env">
          No leaf category with id "{id}" in the static kiosk icon set (
          <code>src/lib/categories.ts</code>). Only leaf categories carry an
          icon — top-level group ids (e.g. "C6") don't. Pick one from the
          dropdown above.
        </p>
      ) : (
        <>
          <section className="flex flex-col gap-3">
            <h2 className="text-xl font-bold">
              Button — {match.groupName} &gt; {match.name}
            </h2>
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
              <ThemedButtonTile
                theme="light"
                small={false}
                id={id}
                name={match.name}
                icon={match.icon}
              />
              <ThemedButtonTile
                theme="light"
                small={true}
                id={id}
                name={match.name}
                icon={match.icon}
              />
              <ThemedButtonTile
                theme="dark"
                small={false}
                id={id}
                name={match.name}
                icon={match.icon}
              />
              <ThemedButtonTile
                theme="dark"
                small={true}
                id={id}
                name={match.name}
                icon={match.icon}
              />
            </div>
          </section>

          <section className="flex flex-col gap-3">
            <h2 className="text-xl font-bold">
              Icon alone ({match.icon}.svg) — sizes × backgrounds
            </h2>
            <div className="overflow-x-auto">
              <table className="border-collapse">
                <thead>
                  <tr>
                    <th className="p-2 text-left text-sm text-ink-muted">
                      size
                    </th>
                    {ICON_BACKGROUNDS.map((bg) => (
                      <th
                        key={bg.label}
                        className="p-2 text-left text-sm text-ink-muted"
                      >
                        {bg.label}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {ICON_SIZES.map((size) => (
                    <tr key={size}>
                      <td className="p-2 font-mono text-sm text-ink-muted">
                        {size}px
                      </td>
                      {ICON_BACKGROUNDS.map((bg) => (
                        <td key={bg.label} className="p-2">
                          <div
                            data-theme={bg.theme}
                            className={`flex items-center justify-center rounded-sm ${bg.className}`}
                            style={{ width: size + 32, height: size + 32 }}
                          >
                            <img
                              src={categoryIconSrc(match.icon)}
                              width={size}
                              height={size}
                            />
                          </div>
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
        </>
      )}
    </div>
  );
}
