import { Suspense } from "react";
import { useSettings } from "../../lib/settings";
import ActivityTimeRange from "../components/ActivityTimeRange";
import ActivityDailyBreakdownDisplay from "../components/ActivityDailyBreakdownDisplay";
import LoadingIndicator from "../../components/LoadingIndicator";
import RelayErrorBoundary from "../../components/RelayErrorBoundary";
import useActivityTimeRange from "../components/useActivityTimeRange";
import { Button } from "../../components/ui/Button";

export default function ActivityDailyBreakdown() {
  const settings = useSettings();
  const {
    startInput,
    endInput,
    setStartInput,
    setEndInput,
    hasValidRange,
    appliedStartTime,
    appliedEndTime,
    isDirty,
    applyRange,
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
        <Button
          variant="primary"
          size="row"
          disabled={!hasValidRange || !isDirty}
          onClick={applyRange}
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
        <RelayErrorBoundary resetKey={`${appliedStartTime}-${appliedEndTime}`}>
          <Suspense fallback={<LoadingIndicator />}>
            <ActivityDailyBreakdownDisplay
              locationId={settings?.locationId || ""}
              startTime={appliedStartTime}
              endTime={appliedEndTime}
            />
          </Suspense>
        </RelayErrorBoundary>
      )}
    </>
  );
}
