import { useEffect, useMemo, useState, type RefObject } from "react";
import { categories } from "../../lib/categories";
import type { Category } from "../../lib/categories";
import { scanViewProps, type ScreenPosition } from "../../styles";
import { useResetScroll } from "../useResetScroll";
import { CategoryIcon } from "../../components/CategoryIcon";
import { inputBase } from "../../components/ui/inputStyles";
import {
  highlightMatches,
  isSearchableQuery,
  searchCategories,
  searchTokens,
  type CategorySearchResult,
} from "../lib/categorySearch";
import {
  SCANNED_HINT,
  useAutoFocusSearch,
  useSearchQuery,
} from "../useCategorySearch";

// Every dimension below is `<px at scale 1> * --cat-scale`; the scale is chosen
// once per kiosk (see categoryScale.ts) and inherited from the scan area. The
// fallback of 1 is the original large size, for use outside the scan area.
export function CategoryButton(props: {
  id: string;
  name: string;
  icon: string;
  onSelect: () => void;
}) {
  const { name, icon, onSelect } = props;

  return (
    <li className="inline-block list-none align-bottom">
      <button
        onClick={onSelect}
        className="m-[calc(12px*var(--cat-scale,1))] box-content flex h-[calc(115px*var(--cat-scale,1))] w-[calc(150px*var(--cat-scale,1))] cursor-pointer flex-col content-start items-stretch rounded-lg border-2 border-line-strong bg-surface-raised p-[calc(10px*var(--cat-scale,1))] text-[calc(18px*var(--cat-scale,1))] leading-[calc(28px*var(--cat-scale,1))] wrap-break-word text-ink active:bg-menu"
      >
        <CategoryIcon
          icon={icon}
          className="mx-auto block size-[calc(70px*var(--cat-scale,1))]"
        />
        {name}
      </button>
    </li>
  );
}

/**
 * The title row above a category list. Shared with the scale probe, which
 * measures it: the drill-down variant (with the back button) is the taller one.
 */
export function CategoriesHeader(props: {
  groupName?: string;
  onBack?: () => void;
  /** Only the probe overrides this, so its text never matches a real screen's. */
  backLabel?: string;
}) {
  return (
    <div
      data-probe="header"
      className="mt-5 flex items-center justify-center gap-3.75 text-3xl"
    >
      {props.groupName !== undefined ? (
        <>
          <button
            className="inline-flex shrink-0 cursor-pointer items-center gap-1.5 rounded-lg border-2 border-line-strong bg-surface-raised px-3.5 py-1.5 align-middle text-ink active:bg-menu"
            onClick={props.onBack}
          >
            <span aria-hidden="true">&#8592;</span>{" "}
            {props.backLabel ?? "Categories"}
          </button>
          <span className="align-middle opacity-60" aria-hidden="true">
            &gt;
          </span>
          <span className="align-middle">{props.groupName}</span>
        </>
      ) : (
        <span className="align-middle">Categories</span>
      )}
    </div>
  );
}

type SearchScope = "role" | "all";

/**
 * The search box under the title row, with the role / all switch when there are
 * results to switch between. Shared with the scale probe, which measures the
 * empty box along with the header (the switch only comes with results, which
 * scroll, so the tiles need no room for it).
 */
export function CategorySearchBar(props: {
  query: string;
  onQueryChange: (query: string) => void;
  inputRef?: RefObject<HTMLInputElement | null>;
  /** Present only inside a role while results show, where the search can be
   * narrowed to that role. */
  scope?: SearchScope;
  onScopeChange?: (scope: SearchScope) => void;
  /** Put the cursor back in the box after ✕ (not on touch kiosks, where that
   * would raise the keyboard over the tiles that come back). */
  focusAfterClear?: boolean;
}) {
  const { query, onQueryChange, inputRef, scope, onScopeChange } = props;
  return (
    <div
      data-probe="search"
      className="mt-3.75 flex flex-col items-center gap-2.5"
    >
      <div className="relative w-full max-w-xl">
        <input
          ref={inputRef}
          type="text"
          role="searchbox"
          aria-label="Search categories"
          placeholder="Search categories"
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          className={`${inputBase} w-full py-2 pr-12 pl-3.5 text-2xl`}
          maxLength={100}
          autoCapitalize="off"
          autoCorrect="off"
          autoComplete="off"
          spellCheck={false}
        />
        {query !== "" && (
          <button
            type="button"
            aria-label="Clear search"
            onClick={() => {
              onQueryChange("");
              // The button is about to disappear; keep typing working.
              if (props.focusAfterClear) {
                inputRef?.current?.focus({ preventScroll: true });
              }
            }}
            className="absolute inset-y-0 right-0 flex w-12 cursor-pointer items-center justify-center text-2xl text-ink opacity-60 active:opacity-100"
          >
            <span aria-hidden="true">&#10005;</span>
          </button>
        )}
      </div>
      {scope !== undefined && onScopeChange !== undefined && (
        <div
          role="group"
          aria-label="Search in"
          className="inline-flex overflow-hidden rounded-lg border-2 border-line-strong text-lg"
        >
          {(
            [
              ["role", "This role"],
              ["all", "All categories"],
            ] as const
          ).map(([value, label]) => (
            <button
              key={value}
              type="button"
              aria-pressed={scope === value}
              onClick={() => onScopeChange(value)}
              className={`flex min-h-11 cursor-pointer items-center px-3.5 text-ink active:bg-menu aria-pressed:bg-navy aria-pressed:text-surface ${value === "all" ? "border-l-2 border-line-strong" : ""}`}
            >
              {label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function Highlighted(props: { text: string; tokens: string[] }) {
  return (
    <>
      {highlightMatches(props.text, props.tokens).map((part, index) =>
        part.match ? (
          <strong key={index}>{part.text}</strong>
        ) : (
          <span key={index}>{part.text}</span>
        ),
      )}
    </>
  );
}

/** One search result: the same look as a category tile, laid out as a row so the
 * whole "Role > Activity" path fits. Always full size: unlike the tiles, the list
 * scrolls, so it has no reason to shrink to fit the screen. */
function SearchResultButton(props: {
  result: CategorySearchResult;
  tokens: string[];
  onSelect: () => void;
}) {
  const { result, tokens, onSelect } = props;
  return (
    <li className="list-none">
      <button
        onClick={onSelect}
        aria-label={`${result.parentName}, ${result.name}`}
        className="mx-auto my-1.5 box-border flex w-full max-w-3xl cursor-pointer items-center gap-3.5 rounded-lg border-2 border-line-strong bg-surface-raised p-2.5 text-left text-[22px] leading-[30px] wrap-break-word text-ink active:bg-menu"
      >
        <CategoryIcon icon={result.icon} className="size-12 shrink-0" />
        <span className="min-w-0">
          <Highlighted text={result.parentName} tokens={tokens} />
          <span className="opacity-60" aria-hidden="true">
            {" "}
            &gt;{" "}
          </span>
          <Highlighted text={result.name} tokens={tokens} />
        </span>
      </button>
    </li>
  );
}

/**
 * What goes under the search box: the screen reader's count, the line saying why
 * a scanned ID was dropped, and, once there is enough typed, the results (or that
 * there are none). The screen shows its own list while `searching` is false.
 */
export function CategorySearchResults(props: {
  query: string;
  searching: boolean;
  scanned: boolean;
  results: CategorySearchResult[];
  /** On a touch kiosk the keyboard covers the bottom of the screen, so the last
   * results need room to scroll above it. */
  keyboardRoom: boolean;
  onSelect: (categoryId: string) => void;
}) {
  const { query, searching, scanned, results } = props;
  const tokens = useMemo(() => searchTokens(query), [query]);
  return (
    <>
      {/* One steady region, so a screen reader reliably hears how many matched
          (or that none did) after each letter; it is not drawn on screen. */}
      <p role="status" className="sr-only">
        {scanned
          ? SCANNED_HINT
          : !searching
            ? ""
            : results.length === 0
              ? `No categories match \u201C${query.trim()}\u201D`
              : `${results.length} ${results.length === 1 ? "category" : "categories"} found`}
      </p>
      {scanned && (
        <p className="mt-6 text-2xl text-ink opacity-60">{SCANNED_HINT}</p>
      )}
      {searching &&
        (results.length > 0 ? (
          <ul
            className={`pl-0 ${props.keyboardRoom ? "pb-[50dvh]" : ""}`}
            aria-label="Search results"
          >
            {results.map((result) => (
              <SearchResultButton
                key={result.id + result.parentId}
                result={result}
                tokens={tokens}
                onSelect={() => props.onSelect(result.id)}
              />
            ))}
          </ul>
        ) : (
          <p className="mt-6 text-2xl text-ink opacity-60">
            No categories match &ldquo;{query.trim()}&rdquo;
          </p>
        ))}
    </>
  );
}

export function Inner(props: {
  onSelectCategory: (uuid: string, categoryId: string) => void;
  uuid: string | null;
  /** Called on mount and whenever the list swaps; the host resets its scroll. */
  onListChange: () => void;
  /** Adds a search box that finds a category by typing instead of drilling in. */
  categorySearch?: boolean;
  /** Put the cursor in the search box when this screen comes on. Off for touch
   * kiosks, where it would raise the keyboard over the tiles every time. */
  autoFocusSearch?: boolean;
  /** Whether this is the screen that is showing. */
  active?: boolean;
}) {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [scope, setScope] = useState<SearchScope>("role");
  const { categorySearch, autoFocusSearch, active } = props;
  // The role / all switch only shows with results, so it starts over on This
  // role whenever the search empties.
  const { query, scanned, changeQuery } = useSearchQuery(() =>
    setScope("role"),
  );
  const searchInputRef = useAutoFocusSearch(
    categorySearch,
    autoFocusSearch,
    active,
  );

  // Drilling into or out of a subcategory, or a new search, swaps the whole list,
  // so start at the top. Runs on mount too, which covers a new uuid (Inner is
  // keyed on it).
  const { onListChange } = props;
  useEffect(() => {
    onListChange();
  }, [onListChange, selectedCategory, query, scope]);

  const searching = !!categorySearch && isSearchableQuery(query);
  const results = useMemo(
    () =>
      searching
        ? searchCategories(query, {
            parentId: scope === "role" ? selectedCategory : null,
          })
        : [],
    [searching, query, scope, selectedCategory],
  );

  const selectedCategoryData = selectedCategory
    ? categories.find((c: Category) => c.id === selectedCategory)
    : null;
  const shownCategories = selectedCategoryData
    ? selectedCategoryData.subcategories || []
    : categories;

  // Sorted as a copy: sorting the list itself would reorder the shared category
  // data, which the search reads to rank groups.
  const sortedCategories = [...shownCategories].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  function back() {
    setSelectedCategory(null);
    setScope("role");
  }

  function select(id: string) {
    if (selectedCategory === null) {
      setSelectedCategory(id);
    } else {
      if (props.uuid === null) {
        throw new Error("UUID is null");
      }
      props.onSelectCategory(props.uuid, id);
    }
  }

  return (
    <>
      <CategoriesHeader groupName={selectedCategoryData?.name} onBack={back} />
      {categorySearch && (
        <CategorySearchBar
          query={query}
          onQueryChange={changeQuery}
          inputRef={searchInputRef}
          focusAfterClear={autoFocusSearch}
          scope={selectedCategoryData && searching ? scope : undefined}
          onScopeChange={setScope}
        />
      )}
      {categorySearch && (
        <CategorySearchResults
          query={query}
          searching={searching}
          scanned={scanned}
          results={results}
          keyboardRoom={!autoFocusSearch}
          onSelect={(categoryId) => {
            if (props.uuid === null) {
              throw new Error("UUID is null");
            }
            props.onSelectCategory(props.uuid, categoryId);
          }}
        />
      )}
      {!searching && (
        <ul className="pl-0">
          {sortedCategories.map((category) => (
            <CategoryButton
              key={category.id}
              id={category.id}
              name={category.name}
              icon={category.icon}
              onSelect={() => select(category.id)}
            />
          ))}
        </ul>
      )}
    </>
  );
}

// we expose this wrapper just so we can reset inner state on UUID change without
// causing the container <div> to remount and lose CSS transition state
export default function ScanScreenCategories(props: {
  onSelectCategory: (uuid: string, categoryId: string) => void;
  screenPosition: ScreenPosition;
  uuid: string | null;
  categorySearch?: boolean;
  autoFocusSearch?: boolean;
}) {
  const { ref: scrollRef, reset: onListChange } =
    useResetScroll<HTMLDivElement>(props.uuid);
  return (
    <div ref={scrollRef} {...scanViewProps(props.screenPosition)}>
      <Inner
        onListChange={onListChange}
        onSelectCategory={props.onSelectCategory}
        key={props.uuid}
        uuid={props.uuid}
        categorySearch={props.categorySearch}
        autoFocusSearch={props.autoFocusSearch}
        active={props.screenPosition === "center"}
      />
    </div>
  );
}
