import { useEffect } from "react";
import type { QuickPickSuggestions } from "../ScanState";
import { categoriesFor, categoryIconSrc } from "../../lib/categories";
import { scanView, scanViewPosition, type ScreenPosition } from "../../styles";
import { Button } from "../../components/ui/Button";

type QuickPickItem = {
  categoryId: string;
  groupName: string;
  name: string;
  icon: string;
  peopleNames?: string[];
};

// The DB `Category` only carries id/name; icon + top-level grouping still live in
// the static kiosk category tree, so look leaf entries up by id (same lookup
// ScanScreenAdjust does to show the confirm-screen category icon). The group
// name is included so e.g. "Training > AIIMS" and "Trainer > AIIMS" — leaf
// names that repeat under different top-level groups — aren't shown as
// identical, indistinguishable buttons.
function findLeafCategory(
  categoryId: string,
  newCategories: boolean,
): { groupName: string; name: string; icon: string } | null {
  for (const top of categoriesFor(newCategories)) {
    for (const sub of top.subcategories || []) {
      if (sub.id === categoryId) {
        return { groupName: top.name, name: sub.name, icon: sub.icon };
      }
    }
  }
  return null;
}

function QuickPickButton(props: {
  item: QuickPickItem;
  newCategories: boolean;
  small?: boolean;
  onSelect: () => void;
}) {
  const { item, newCategories, small, onSelect } = props;
  const iconSrc = categoryIconSrc(item.icon, newCategories);

  return (
    <li className="flex list-none">
      <button
        onClick={onSelect}
        className={
          small
            ? "m-2 box-content flex min-h-21 w-28.75 cursor-pointer flex-col content-start rounded-lg border-2 border-line-strong bg-surface-raised p-1.75 text-sm wrap-break-word text-ink active:bg-menu"
            : "m-2 box-content flex min-h-28.75 w-37.5 cursor-pointer flex-col content-start rounded-lg border-2 border-line-strong bg-surface-raised p-2.5 text-lg wrap-break-word text-ink active:bg-menu"
        }
      >
        <img
          src={iconSrc}
          className={`mx-auto block ${small ? "max-h-12 max-w-12" : ""}`}
        />
        <span className={`opacity-60 ${small ? "text-xs" : "text-sm"}`}>
          {item.groupName}
        </span>
        <span
          className={`line-clamp-2 font-semibold ${small ? "min-h-10" : "min-h-14"}`}
        >
          {item.name}
        </span>
        {item.peopleNames && item.peopleNames.length > 0 && (
          <span className="mt-auto text-sm opacity-60">
            {item.peopleNames.join(", ")}
          </span>
        )}
      </button>
    </li>
  );
}

function QuickPickSection(props: {
  title: string;
  description: string;
  items: QuickPickItem[];
  newCategories: boolean;
  small?: boolean;
  onSelect: (categoryId: string) => void;
}) {
  if (props.items.length === 0) {
    return null;
  }

  return (
    <div className="mx-auto mt-6 max-w-[95%]">
      <h2 className="m-0 text-center text-lg font-semibold tracking-wide uppercase">
        {props.title}
      </h2>
      <p className="mx-auto mt-0.5 mb-0 max-w-100 text-center text-sm opacity-60">
        {props.description}
      </p>
      <ul className="flex flex-wrap justify-center pl-0">
        {props.items.map((item) => (
          <QuickPickButton
            key={item.categoryId}
            item={item}
            newCategories={props.newCategories}
            small={props.small}
            onSelect={() => props.onSelect(item.categoryId)}
          />
        ))}
      </ul>
    </div>
  );
}

// Suggestions name categories by id only; drop any the kiosk's static tree
// doesn't know about (a category retired from the tree, or one only the admin
// UI uses) rather than rendering a button with no name or icon.
function toItems(
  categories: QuickPickSuggestions["location"],
  newCategories: boolean,
): QuickPickItem[] {
  return categories
    .map((entry): QuickPickItem | null => {
      const leaf = findLeafCategory(entry.categoryId, newCategories);
      if (!leaf) {
        return null;
      }
      return {
        categoryId: entry.categoryId,
        groupName: leaf.groupName,
        name: leaf.name,
        icon: leaf.icon,
        peopleNames: entry.peopleNames,
      };
    })
    .filter((item): item is QuickPickItem => item !== null);
}

function Inner(props: {
  suggestions: QuickPickSuggestions;
  newCategories: boolean;
  smallCategories?: boolean;
  onSelectCategory: (categoryId: string) => void;
  onSkip: () => void;
}) {
  const { newCategories, suggestions } = props;

  const locationItems = toItems(suggestions.location, newCategories);
  const personItems = toItems(suggestions.person, newCategories);

  const isEmpty = locationItems.length === 0 && personItems.length === 0;

  const { onSkip } = props;
  useEffect(() => {
    // Nothing to quick-pick from (new location/person) — skip straight to the
    // full category tree instead of showing two empty sections.
    if (isEmpty) {
      onSkip();
    }
  }, [isEmpty, onSkip]);

  if (isEmpty) {
    return null;
  }

  return (
    <>
      <div className="mt-5 flex items-center justify-center gap-3.75 text-[2em]">
        <span className="align-middle">Quick pick</span>
      </div>
      <QuickPickSection
        title="This location"
        description="Popular here recently"
        items={locationItems}
        newCategories={newCategories}
        small={props.smallCategories}
        onSelect={props.onSelectCategory}
      />
      <QuickPickSection
        title="You"
        description="Your recent picks"
        items={personItems}
        newCategories={newCategories}
        small={props.smallCategories}
        onSelect={props.onSelectCategory}
      />
      <div className="mt-8 pb-2 text-center">
        <Button
          variant="kiosk"
          size="bare"
          className="inline-flex items-center gap-2 px-7 py-3 text-[1.4em]"
          onClick={props.onSkip}
        >
          More categories
          <span aria-hidden="true">&#8594;</span>
        </Button>
      </div>
    </>
  );
}

// we expose this wrapper just so we can reset inner state on UUID change without
// causing the container <div> to remount and lose CSS transition state
export default function ScanScreenQuickPick(props: {
  onSelectCategory: (uuid: string, categoryId: string) => void;
  onSkip: () => void;
  screenPosition: ScreenPosition;
  uuid: string | null;
  /** Comes back with the sign-out itself; null if the server had none to give. */
  suggestions: QuickPickSuggestions | null;
  smallCategories?: boolean;
  newCategories?: boolean;
}) {
  const { uuid, suggestions, onSelectCategory } = props;

  return (
    <div className={`${scanView} ${scanViewPosition[props.screenPosition]}`}>
      {uuid && suggestions && (
        <Inner
          key={uuid}
          suggestions={suggestions}
          smallCategories={props.smallCategories}
          newCategories={!!props.newCategories}
          onSelectCategory={(categoryId) => onSelectCategory(uuid, categoryId)}
          onSkip={props.onSkip}
        />
      )}
    </div>
  );
}
