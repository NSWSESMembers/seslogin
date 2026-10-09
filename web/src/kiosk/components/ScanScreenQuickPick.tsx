import { useEffect } from "react";
import type { QuickPickSuggestions } from "../ScanState";
import { findLeafCategory } from "../../lib/categories";
import { useResetScroll } from "../useResetScroll";
import { scanViewProps, type ScreenPosition } from "../../styles";
import { Button } from "../../components/ui/Button";
import { CategoryIcon } from "../../components/CategoryIcon";

type QuickPickItem = {
  categoryId: string;
  groupName: string;
  name: string;
  icon: string;
  peopleNames?: string[];
};

// Sized from the same per-kiosk `--cat-scale` as the category buttons (see
// categoryScale.ts and ScanScreenCategories).
function QuickPickButton(props: { item: QuickPickItem; onSelect: () => void }) {
  const { item, onSelect } = props;

  return (
    <li className="flex list-none">
      <button
        onClick={onSelect}
        className="m-[calc(8px*var(--cat-scale,1))] box-content flex min-h-[calc(115px*var(--cat-scale,1))] w-[calc(150px*var(--cat-scale,1))] cursor-pointer flex-col content-start items-stretch rounded-lg border-2 border-line-strong bg-surface-raised p-[calc(10px*var(--cat-scale,1))] text-[calc(18px*var(--cat-scale,1))] leading-[calc(28px*var(--cat-scale,1))] wrap-break-word text-ink active:bg-menu"
      >
        <CategoryIcon
          icon={item.icon}
          className="mx-auto block size-[calc(70px*var(--cat-scale,1))]"
        />
        <span className="text-[calc(14px*var(--cat-scale,1))] leading-[calc(20px*var(--cat-scale,1))] opacity-60">
          {item.groupName}
        </span>
        <span className="line-clamp-2 min-h-[calc(56px*var(--cat-scale,1))] font-semibold">
          {item.name}
        </span>
        {item.peopleNames && item.peopleNames.length > 0 && (
          <span className="mt-auto text-[calc(14px*var(--cat-scale,1))] leading-[calc(20px*var(--cat-scale,1))] opacity-60">
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
function toItems(entries: QuickPickSuggestions["location"]): QuickPickItem[] {
  return entries
    .map((entry): QuickPickItem | null => {
      const leaf = findLeafCategory(entry.categoryId);
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
  onSelectCategory: (categoryId: string) => void;
  onSkip: () => void;
}) {
  const { suggestions } = props;

  const locationItems = toItems(suggestions.location);
  const personItems = toItems(suggestions.person);

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
      <div className="mt-5 flex items-center justify-center gap-3.75 text-3xl">
        <span className="align-middle">Quick pick</span>
      </div>
      <QuickPickSection
        title="This location"
        description="Popular here recently"
        items={locationItems}
        onSelect={props.onSelectCategory}
      />
      <QuickPickSection
        title="You"
        description="Your recent picks"
        items={personItems}
        onSelect={props.onSelectCategory}
      />
      <div className="mt-8 pb-2 text-center">
        <Button
          variant="kiosk"
          size="bare"
          className="inline-flex items-center gap-2 px-7 py-3 text-2xl"
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
}) {
  const { uuid, suggestions, onSelectCategory } = props;

  const { ref: scrollRef } = useResetScroll<HTMLDivElement>(uuid);

  return (
    <div ref={scrollRef} {...scanViewProps(props.screenPosition)}>
      {uuid && suggestions && (
        <Inner
          key={uuid}
          suggestions={suggestions}
          onSelectCategory={(categoryId) => onSelectCategory(uuid, categoryId)}
          onSkip={props.onSkip}
        />
      )}
    </div>
  );
}
