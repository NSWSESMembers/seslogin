import { Suspense, useState } from "react";
import { useSettings } from "../../lib/settings";
import ActivityCategorySelector from "../components/ActivityCategorySelector";
import ActivityTimeRange from "../components/ActivityTimeRange";
import ActivityTotalsDisplay from "../components/ActivityTotalsDisplay";
import LoadingIndicator from "../../components/LoadingIndicator";
import useActivityTimeRange from "../components/useActivityTimeRange";

export default function ActivityTotals() {
  const settings = useSettings();
  const [category, setCategory] = useState("");
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
      <Suspense fallback={<LoadingIndicator />}>
        <ActivityCategorySelector value={category} onChange={setCategory} />
      </Suspense>
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
            category={category || undefined}
          />
        </Suspense>
      )}
    </>
  );
}
