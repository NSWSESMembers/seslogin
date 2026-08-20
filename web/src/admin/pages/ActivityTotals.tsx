import { Suspense, useState } from "react";
import { useSettings } from "../../lib/settings";
import ActivityCategorySelector from "../components/ActivityCategorySelector";
import ActivityTimeRange from "../components/ActivityTimeRange";
import ActivityTotalsDisplay from "../components/ActivityTotalsDisplay";
import LoadingIndicator from "../../components/LoadingIndicator";
import useActivityTimeRange from "../components/useActivityTimeRange";
import { Button } from "../../components/ui/Button";

function sameCategoryIds(a: ReadonlyArray<string>, b: ReadonlyArray<string>) {
  if (a.length !== b.length) return false;
  const bSet = new Set(b);
  return a.every((id) => bSet.has(id));
}

export default function ActivityTotals() {
  const settings = useSettings();
  const [categoryIds, setCategoryIds] = useState<string[]>([]);
  const [appliedCategoryIds, setAppliedCategoryIds] = useState<string[]>([]);
  const {
    startInput,
    endInput,
    setStartInput,
    setEndInput,
    hasValidRange,
    appliedStartTime,
    appliedEndTime,
    isDirty: isTimeRangeDirty,
    applyRange,
  } = useActivityTimeRange();

  const isDirty =
    isTimeRangeDirty || !sameCategoryIds(categoryIds, appliedCategoryIds);

  function applyFilters() {
    applyRange();
    setAppliedCategoryIds(categoryIds);
  }

  return (
    <>
      <ActivityTimeRange
        startInput={startInput}
        endInput={endInput}
        onStartChange={setStartInput}
        onEndChange={setEndInput}
      />
      <div className="mb-4 flex flex-wrap items-center justify-center gap-4">
        <Suspense fallback={null}>
          <ActivityCategorySelector
            value={categoryIds}
            onChange={setCategoryIds}
          />
        </Suspense>
        <Button
          variant="primary"
          size="row"
          disabled={!hasValidRange || !isDirty}
          onClick={applyFilters}
        >
          Update results
        </Button>
      </div>
      {!hasValidRange && (
        <p className="font-bold text-red-600">
          Start time must be before end time.
        </p>
      )}

      {hasValidRange && (
        <Suspense fallback={<LoadingIndicator />}>
          <ActivityTotalsDisplay
            locationId={settings?.locationId || ""}
            startTime={appliedStartTime}
            endTime={appliedEndTime}
            categories={appliedCategoryIds}
          />
        </Suspense>
      )}
    </>
  );
}
