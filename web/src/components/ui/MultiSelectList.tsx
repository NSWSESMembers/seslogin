import {
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { tw } from "../../lib/tw";
import { inputBase } from "./inputStyles";

export interface MultiSelectOption {
  id: string;
  name: string;
  /** Optional secondary line, also searchable. */
  hint?: string;
  disabled?: boolean;
}

interface MultiSelectListProps {
  /** Order respected — never re-sorted by this component. */
  options: ReadonlyArray<MultiSelectOption>;
  value: ReadonlySet<string>;
  onChange: (next: ReadonlySet<string>) => void;
  /** When given, one hidden input per selection is emitted for FormData call sites. */
  name?: string;
  /** e.g. "locations" — used in the placeholder, counts, legend, live region. */
  itemLabel: string;
  id?: string;
  /** Visible rows before scrolling. Default 8. */
  rows?: number;
  /** Shown when `options` is empty. */
  emptyMessage?: ReactNode;
  disabled?: boolean;
}

function tokenize(query: string): string[] {
  return query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
}

function optionMatches(option: MultiSelectOption, tokens: string[]): boolean {
  if (tokens.length === 0) return true;
  const haystack = `${option.name} ${option.hint ?? ""}`.toLocaleLowerCase();
  return tokens.every((token) => haystack.includes(token));
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Wraps token matches in <mark>. Falls back to plain text when there is nothing to highlight. */
function highlight(text: string, tokens: string[]): ReactNode {
  if (tokens.length === 0) return text;
  const pattern = tokens.map(escapeRegExp).join("|");
  const re = new RegExp(`(${pattern})`, "gi");
  const parts = text.split(re);
  if (parts.length === 1) return text;
  // `text.split(re)` with one capturing group alternates non-match/match
  // segments starting with a (possibly empty) non-match, so odd indices are
  // always the matched runs.
  return parts.map((part, i) =>
    i % 2 === 1 ? (
      <mark key={i} className="rounded-xs bg-menu/30 text-ink">
        {part}
      </mark>
    ) : (
      part
    ),
  );
}

const filterInputClassName = [inputBase, tw`w-full text-sm`].join(" ");

const rowBase = tw`flex min-h-8 cursor-pointer items-start gap-2 border-b border-line-faint px-2 py-1.5 last:border-b-0 hover:bg-brand/5 has-checked:bg-brand/5`;
const rowDisabled = tw`cursor-not-allowed opacity-50`;

const bulkActionClassName = tw`text-ink-muted underline decoration-dotted hover:text-ink disabled:cursor-not-allowed disabled:no-underline disabled:opacity-50`;

/**
 * A search input that filters a native checkbox group. Native checkboxes
 * (rather than an ARIA combobox/listbox) keep real labels, real focus, native
 * `FormData` participation, and the `has-checked:` Tailwind hook; typeahead
 * keyboard flow is layered on top via a roving tabIndex.
 */
export default function MultiSelectList({
  options,
  value,
  onChange,
  name,
  itemLabel,
  id,
  rows = 8,
  emptyMessage,
  disabled = false,
}: MultiSelectListProps) {
  const [filter, setFilter] = useState("");
  const [showSelectedOnly, setShowSelectedOnly] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);

  const filterInputRef = useRef<HTMLInputElement>(null);
  const rowRefs = useRef<Map<string, HTMLInputElement>>(new Map());

  const generatedId = useId();
  const rootId = id ?? generatedId;
  const listId = `${rootId}-list`;

  const tokens = useMemo(() => tokenize(filter), [filter]);

  // Matches the current filter text only — independent of "show selected
  // only" — so the bulk actions below stay scoped to the filter even while
  // that toggle further narrows what's rendered.
  const matchingOptions = useMemo(
    () => options.filter((option) => optionMatches(option, tokens)),
    [options, tokens],
  );

  const visibleOptions = useMemo(
    () =>
      showSelectedOnly
        ? matchingOptions.filter((option) => value.has(option.id))
        : matchingOptions,
    [matchingOptions, showSelectedOnly, value],
  );

  const matchingIds = useMemo(
    () =>
      matchingOptions
        .filter((option) => !option.disabled)
        .map((option) => option.id),
    [matchingOptions],
  );

  const activeIndex = visibleOptions.findIndex(
    (option) => option.id === activeId,
  );
  const effectiveActiveId =
    activeIndex >= 0 ? activeId : (visibleOptions[0]?.id ?? null);

  function focusRow(rowId: string | null) {
    if (rowId == null) return;
    setActiveId(rowId);
    rowRefs.current.get(rowId)?.focus();
  }

  function toggleOption(optionId: string) {
    if (disabled) return;
    const option = options.find((o) => o.id === optionId);
    if (!option || option.disabled) return;
    const next = new Set(value);
    if (next.has(optionId)) {
      next.delete(optionId);
    } else {
      next.add(optionId);
    }
    onChange(next);
  }

  function selectMatching() {
    if (matchingIds.length === 0) return;
    const next = new Set(value);
    matchingIds.forEach((optionId) => next.add(optionId));
    onChange(next);
  }

  function deselectMatching() {
    if (matchingIds.length === 0) return;
    const next = new Set(value);
    matchingIds.forEach((optionId) => next.delete(optionId));
    onChange(next);
  }

  function clearAll() {
    if (value.size === 0) return;
    onChange(new Set());
  }

  function handleFilterKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        focusRow(visibleOptions[0]?.id ?? null);
        break;
      case "ArrowUp":
        e.preventDefault();
        focusRow(visibleOptions[visibleOptions.length - 1]?.id ?? null);
        break;
      case "Enter":
        // Never let the surrounding form submit on Enter here.
        e.preventDefault();
        if (visibleOptions.length === 1) {
          toggleOption(visibleOptions[0].id);
        } else {
          focusRow(visibleOptions[0]?.id ?? null);
        }
        break;
      case "Escape":
        if (filter !== "") {
          e.stopPropagation();
          setFilter("");
        }
        // Otherwise let it bubble so a popover host can close.
        break;
      default:
        break;
    }
  }

  function handleRowKeyDown(e: KeyboardEvent<HTMLInputElement>, index: number) {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        if (index + 1 < visibleOptions.length) {
          focusRow(visibleOptions[index + 1].id);
        }
        break;
      case "ArrowUp":
        e.preventDefault();
        if (index === 0) {
          filterInputRef.current?.focus();
        } else {
          focusRow(visibleOptions[index - 1].id);
        }
        break;
      case "Home":
        e.preventDefault();
        focusRow(visibleOptions[0]?.id ?? null);
        break;
      case "End":
        e.preventDefault();
        focusRow(visibleOptions[visibleOptions.length - 1]?.id ?? null);
        break;
      case "Enter":
        e.preventDefault();
        toggleOption(visibleOptions[index].id);
        break;
      case "Escape":
        e.stopPropagation();
        filterInputRef.current?.focus();
        break;
      default:
        break;
    }
  }

  const hasOptions = options.length > 0;

  let emptyRowsMessage: string | null = null;
  if (hasOptions && visibleOptions.length === 0) {
    emptyRowsMessage =
      matchingOptions.length === 0
        ? `No ${itemLabel} match “${filter}”.`
        : filter
          ? `No selected ${itemLabel} match “${filter}”.`
          : `No ${itemLabel} selected.`;
  }

  return (
    <>
      <fieldset
        id={rootId}
        disabled={disabled}
        className={tw`flex flex-col rounded-md border border-line bg-surface`}
      >
        <legend className="sr-only">{itemLabel}</legend>

        <div
          aria-live="polite"
          className="sr-only"
        >{`${matchingOptions.length} of ${options.length} ${itemLabel} match the filter, ${value.size} selected.`}</div>

        {!hasOptions ? (
          <div className="p-3 text-sm text-ink-muted">{emptyMessage}</div>
        ) : (
          <>
            <div className="sticky top-0 z-10 flex flex-col gap-2 rounded-t-md border-b border-line bg-surface-raised p-2">
              <input
                ref={filterInputRef}
                type="text"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                onKeyDown={handleFilterKeyDown}
                placeholder={`Filter ${itemLabel}…`}
                aria-label={`Filter ${itemLabel}`}
                aria-controls={listId}
                className={filterInputClassName}
              />
              <div className="text-xs text-ink-muted">
                {value.size} of {options.length} selected
              </div>
              <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs">
                <button
                  type="button"
                  onClick={selectMatching}
                  disabled={matchingIds.length === 0}
                  className={bulkActionClassName}
                >
                  Select {matchingIds.length} matching
                </button>
                <button
                  type="button"
                  onClick={deselectMatching}
                  disabled={matchingIds.length === 0}
                  className={bulkActionClassName}
                >
                  Deselect {matchingIds.length} matching
                </button>
                <button
                  type="button"
                  onClick={clearAll}
                  disabled={value.size === 0}
                  className={bulkActionClassName}
                >
                  Clear all ({value.size})
                </button>
              </div>
              <label className="flex items-center gap-1.5 text-xs text-ink">
                <input
                  type="checkbox"
                  checked={showSelectedOnly}
                  onChange={(e) => setShowSelectedOnly(e.target.checked)}
                />
                Show selected only ({value.size})
              </label>
            </div>

            {emptyRowsMessage !== null ? (
              <p className="p-3 text-center text-sm text-ink-muted">
                {emptyRowsMessage}
              </p>
            ) : (
              <div
                id={listId}
                className="flex flex-col overflow-y-auto"
                style={{ maxHeight: `min(${rows * 2.25}rem, 70vh)` }}
              >
                {visibleOptions.map((option, index) => {
                  const checked = value.has(option.id);
                  const isActive = option.id === effectiveActiveId;
                  return (
                    <label
                      key={option.id}
                      className={
                        option.disabled ? `${rowBase} ${rowDisabled}` : rowBase
                      }
                    >
                      <input
                        ref={(el) => {
                          if (el) rowRefs.current.set(option.id, el);
                          else rowRefs.current.delete(option.id);
                        }}
                        type="checkbox"
                        className="mt-0.5 shrink-0"
                        checked={checked}
                        disabled={disabled || option.disabled}
                        tabIndex={isActive ? 0 : -1}
                        onChange={() => toggleOption(option.id)}
                        onFocus={() => setActiveId(option.id)}
                        onKeyDown={(e) => handleRowKeyDown(e, index)}
                      />
                      <span className="flex flex-col text-sm text-ink">
                        <span>{highlight(option.name, tokens)}</span>
                        {option.hint && (
                          <span className="text-xs text-ink-muted">
                            {highlight(option.hint, tokens)}
                          </span>
                        )}
                      </span>
                    </label>
                  );
                })}
              </div>
            )}
          </>
        )}
      </fieldset>

      {/* Outside the fieldset: a disabled fieldset drops its descendants
          from FormData, which would otherwise silently erase the selection
          whenever `disabled` is set. */}
      {name &&
        [...value].map((selectedId) => (
          <input
            key={selectedId}
            type="hidden"
            name={name}
            value={selectedId}
          />
        ))}
    </>
  );
}
