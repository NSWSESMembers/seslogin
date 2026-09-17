import { useRef, useState } from "react";
import { graphql } from "relay-runtime";
import type { ActivityCategorySelectorQuery } from "./__generated__/ActivityCategorySelectorQuery.graphql";
import { inputBase } from "../../components/ui/inputStyles";
import { Button } from "../../components/ui/Button";
import MultiSelectList from "../../components/ui/MultiSelectList";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";

interface ActivityCategorySelectorProps {
  value: ReadonlyArray<string>;
  onChange: (categoryIds: string[]) => void;
  // When true, each checkbox/select-all/clear edit calls onChange right
  // away and no inner Apply button is shown — for callers that already
  // gate the actual query behind their own "Update results" button, so a
  // second, inner Apply would just be a confusing extra step.
  applyImmediately?: boolean;
}

export default function ActivityCategorySelector({
  value,
  onChange,
  applyImmediately = false,
}: ActivityCategorySelectorProps) {
  const data = useRetryableLazyLoadQuery<ActivityCategorySelectorQuery>(
    graphql`
      query ActivityCategorySelectorQuery @throwOnFieldError {
        categories {
          id
          name
        }
      }
    `,
    {},
  );

  const categories = data.categories
    .toSorted((a, b) => a.name.localeCompare(b.name))
    .map((category) => ({ id: category.id, name: category.name }));

  const detailsRef = useRef<HTMLDetailsElement>(null);
  // Local selection edited while the dropdown is open. When !applyImmediately,
  // it's only committed to `onChange` (and thus re-fetches the report) when
  // the user clicks Apply, so ticking several boxes doesn't trigger a query
  // per click. When applyImmediately, it's committed on every edit instead —
  // for callers with their own outer "Update results" gate, where this is
  // just the draft value and doesn't itself trigger a fetch.
  const [pending, setPending] = useState<ReadonlySet<string>>(
    () => new Set(value),
  );

  function commit(next: ReadonlySet<string>) {
    setPending(next);
    if (applyImmediately) {
      onChange([...next]);
    }
  }

  function handleToggle() {
    // Re-sync pending selection from the applied value whenever the dropdown
    // opens, so a close-without-applying discards any edits made last time.
    if (detailsRef.current?.open) {
      setPending(new Set(value));
    }
  }

  function apply() {
    onChange([...pending]);
    if (detailsRef.current) {
      detailsRef.current.open = false;
    }
  }

  return (
    <details
      ref={detailsRef}
      className="inline-block text-left"
      onToggle={handleToggle}
    >
      <summary
        className={[inputBase, "h-7.5 cursor-pointer text-sm leading-6"]
          .filter(Boolean)
          .join(" ")}
      >
        Categories: {value.length === 0 ? "All" : `${value.length} selected`}
      </summary>
      <div className="mt-2 flex w-max min-w-64 flex-col gap-1">
        <MultiSelectList
          options={categories}
          value={pending}
          onChange={commit}
          itemLabel="categories"
          rows={6}
        />
        {!applyImmediately && (
          <div className="flex justify-end">
            <Button size="row" onClick={apply}>
              Apply
            </Button>
          </div>
        )}
      </div>
    </details>
  );
}
