import { Suspense, useState } from "react";
import { useSettings } from "../../lib/settings";
import ActivityCategorySelector from "../components/ActivityCategorySelector";
import ActivityTimeRange from "../components/ActivityTimeRange";
import ActivityTotalsDisplay from "../components/ActivityTotalsDisplay";
import LoadingIndicator from "../../components/LoadingIndicator";
import useActivityTimeRange from "../components/useActivityTimeRange";

export default function ActivityTotals() {
  const settings = useSettings();
  const [categoryIds, setCategoryIds] = useState<string[]>([]);
  const {
    startInput,
    endInput,
    setStartInput,
    setEndInput,
    hasValidRange,
    queryStartTime,
    queryEndTime,
  } = useActivityTimeRange();

  return (
    <>
      <ActivityTimeRange
        startInput={startInput}
        endInput={endInput}
        onStartChange={setStartInput}
        onEndChange={setEndInput}
      />
      <div className="mb-4 flex justify-center">
        <Suspense fallback={<LoadingIndicator />}>
          <ActivityCategorySelector
            value={categoryIds}
            onChange={setCategoryIds}
          />
        </Suspense>
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
            startTime={queryStartTime}
            endTime={queryEndTime}
            categories={categoryIds}
          />
        </Suspense>
      )}
    </>
  );
}
