import { useState, type CSSProperties } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { formatTime, formatTimeDiff } from "../../lib/time";
import { unwrapCatch } from "../../lib/relayCatch";
import { graphql, isValueResult, readInlineData } from "relay-runtime";
import { useMutation } from "react-relay";
import type { ActivityListTableDeleteMutation } from "./__generated__/ActivityListTableDeleteMutation.graphql";
import type {
  ActivityListTableRemindMutation,
  ActivityListTableRemindMutation$data,
} from "./__generated__/ActivityListTableRemindMutation.graphql";
import type {
  ActivityListTable_period$data,
  ActivityListTable_period$key,
} from "./__generated__/ActivityListTable_period.graphql";
import bulletOrange from "../../assets/bullet-orange.svg";
import bulletGreen from "../../assets/bullet-green.svg";
import { useUserInfo } from "./useUserInfo";
import { useNotify } from "./useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";
import CommentIndicator from "./CommentIndicator";

type Firstcol = "location" | "person";

// Colocated data dependency for a single activity row: only the fields this table
// actually renders. The display name (person vs location) differs per page, so each
// page colocates its own name fragment and supplies a `getRowLabel` that reads it from
// the same period ref. Marked @inline so the day-grouping loop and Row can read the
// fields via readInlineData outside of useFragment.
const activityListTablePeriod = graphql`
  fragment ActivityListTable_period on Period @inline {
    id
    personId
    startTime
    endTime
    comment
    nitcExportStatus
    nitcEventId
    # @catch on every relation below: a dangling reference (deleted session,
    # deleted category) must not throw @throwOnFieldError's whole-query error
    # and hide every other row — Row unwraps these with unwrapCatch, which
    # throws, so a per-row ErrorBoundary degrades just that one row instead.
    signedInSession @catch {
      id
      name
    }
    signedOutSession @catch {
      id
      name
    }
    category @catch {
      id
      name
      isVirtual
    }
  }
`;

// Dotted underline hint shown on a sign-in/out time when a session name is
// available in its `title` tooltip, signalling there's more to see on hover.
const sessionHintStyle: CSSProperties = {
  textDecoration: "underline dotted",
  cursor: "help",
};

type Period = ActivityListTable_period$data;
// Each row keeps the original fragment ref (passed to the page's getRowLabel) alongside
// the data already read for this table's own fields.
type Entry<T extends ActivityListTable_period$key> = { ref: T; data: Period };

function Section<T extends ActivityListTable_period$key>({
  day,
  entries,
  getRowLabel,
  isDev,
  showSplit,
}: {
  day: string;
  entries: ReadonlyArray<Entry<T>>;
  getRowLabel: (p: T) => string;
  isDev: boolean;
  showSplit: boolean;
}) {
  const colSpan = isDev ? 8 : 7;
  const periodCount = entries.length;
  const uniqueMemberCount = new Set(
    entries.map((entry) => entry.data.personId).filter(Boolean),
  ).size;
  const periodLabel = periodCount === 1 ? "period" : "periods";
  const memberLabel = uniqueMemberCount === 1 ? "member" : "members";
  // Non-throwing here: this is a summary count spanning every row in the
  // section, not a single row's own render, so a failed category just counts
  // as non-virtual rather than blanking the whole day's summary. The row
  // itself still shows its own "unable to load" state below.
  const virtualCount = entries.filter((entry) => {
    const category = entry.data.category;
    return isValueResult(category) && category.value?.isVirtual;
  }).length;
  const nonVirtualCount = periodCount - virtualCount;

  return (
    <>
      <tr>
        <Th section colSpan={colSpan}>
          <div>{day}</div>
          <div className="font-normal text-ink-muted">
            {periodCount} {periodLabel}
            {showSplit
              ? ` — ${virtualCount} virtual / ${nonVirtualCount} non-virtual`
              : ""}
            , {uniqueMemberCount} unique {memberLabel}
          </div>
        </Th>
      </tr>
      <tr>
        <td className="h-0.75" colSpan={colSpan}></td>
      </tr>
      {entries.map((entry, idx) => (
        <ErrorBoundary
          key={entry.data.id}
          fallbackRender={() => (
            <tr>
              <td colSpan={colSpan} className="font-bold text-red-600">
                Unable to load period with ID {entry.data.id}
              </td>
            </tr>
          )}
          onError={(error) =>
            console.error(
              `Failed to render activity row ${entry.data.id}:`,
              error,
            )
          }
        >
          <Row
            entry={entry}
            idx={idx}
            getRowLabel={getRowLabel}
            isDev={isDev}
          />
        </ErrorBoundary>
      ))}
    </>
  );
}

function Row<T extends ActivityListTable_period$key>({
  entry,
  idx,
  getRowLabel,
  isDev,
}: {
  entry: Entry<T>;
  idx: number;
  getRowLabel: (p: T) => string;
  isDev: boolean;
}) {
  const period = entry.data;
  // Throwing unwraps: a failure here is caught by the per-row ErrorBoundary
  // this Row is always rendered inside (see Section), degrading just this row.
  const category = unwrapCatch(period.category);
  const signedInSession = unwrapCatch(period.signedInSession);
  const signedOutSession = unwrapCatch(period.signedOutSession);
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] =
    useMutation<ActivityListTableDeleteMutation>(graphql`
      mutation ActivityListTableDeleteMutation($id: ID!) {
        deletePeriod(id: $id)
      }
    `);
  const [commitRemind, isRemindInFlight] =
    useMutation<ActivityListTableRemindMutation>(graphql`
      mutation ActivityListTableRemindMutation($id: ID!) {
        sendPeriodEditLink(id: $id)
      }
    `);

  async function sendReminder() {
    const yes = confirm(
      `Email ${getRowLabel(entry.ref)} a link to check and correct this time entry?`,
    );
    if (!yes) return;
    try {
      const result = await new Promise<ActivityListTableRemindMutation$data>(
        (resolve, reject) => {
          commitRemind({
            variables: { id: period.id },
            onCompleted: resolve,
            onError: reject,
          });
        },
      );
      notifySuccess(`Reminder sent to ${result.sendPeriodEditLink}`);
    } catch (err) {
      notifyError(err, "Couldn't send the reminder");
    }
  }

  async function deletePeriod() {
    const yes = confirm(
      `Are you sure you want to delete this activity entry? This action cannot be undone.`,
    );
    if (yes) {
      try {
        await new Promise((resolve, reject) => {
          commitMutation({
            variables: { id: period.id },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.delete(period.id);
            },
          });
        });
        notifySuccess("Activity entry deleted");
      } catch (err) {
        notifyError(err, "Couldn't delete activity entry");
      }
    }
  }

  const start = new Date(period.startTime * 1000);
  const end = period.endTime ? new Date(period.endTime * 1000) : undefined;
  const timeDiff = period.endTime ? formatTimeDiff(start, end!) : "";

  const nitcStatus = period.nitcExportStatus;
  let nitcBullet: string | null = null;
  let bulletTitle = "";
  if (nitcStatus === "PENDING") {
    nitcBullet = bulletOrange;
    bulletTitle = "Not yet exported to NITC";
  } else if (nitcStatus === "SYNCED") {
    nitcBullet = bulletGreen;
    bulletTitle = `Exported into NITC event ${period.nitcEventId}`;
  }

  const beaconUrl = import.meta.env.VITE_BEACON_URL;
  const nitcLink =
    period.nitcEventId && beaconUrl
      ? `${beaconUrl}/nitc/${period.nitcEventId}`
      : null;

  return (
    <tr className={idx % 2 === 0 ? "bg-surface-raised" : undefined}>
      <Td center>
        {nitcBullet ? (
          nitcLink ? (
            <a href={nitcLink} target="_blank" rel="noreferrer">
              <img
                src={nitcBullet}
                alt=""
                title={bulletTitle}
                width={12}
                height={12}
                className="max-w-none align-middle"
              />
            </a>
          ) : (
            <img
              src={nitcBullet}
              alt=""
              title={bulletTitle}
              width={12}
              height={12}
              className="max-w-none align-middle"
            />
          )
        ) : null}
      </Td>
      {isDev && <Td className="font-mono text-[0.85em]">{period.id}</Td>}
      <Td>{getRowLabel(entry.ref)}</Td>
      <Td
        title={signedInSession?.name ?? undefined}
        style={signedInSession ? sessionHintStyle : undefined}
      >
        {formatTime(start)}
      </Td>
      <Td
        title={signedOutSession?.name ?? undefined}
        style={signedOutSession ? sessionHintStyle : undefined}
      >
        {end ? formatTime(end) : ""}
      </Td>
      <Td>{timeDiff}</Td>
      <Td>
        <span className="inline-flex items-center gap-1.5">
          {category?.name}
          {period.comment && <CommentIndicator comment={period.comment} />}
        </span>
      </Td>
      <Td options>
        <div className="flex justify-end gap-1">
          <ButtonLink size="row" to={`/admin/activity/${period.id}`}>
            Edit
          </ButtonLink>
          {/* Still being trialled, so dev-tagged users only. The guest check is
              the same as Edit's: a guest has no member record to email. */}
          {isDev && period.personId != null && (
            <Button
              size="row"
              onClick={sendReminder}
              disabled={isRemindInFlight}
              title="Email this member a link to check and correct their time entry"
            >
              {isRemindInFlight ? "Sending…" : "Remind"} [dev-only]
            </Button>
          )}
          <Button
            size="row"
            variant="danger"
            onClick={deletePeriod}
            disabled={isMutationInFlight}
          >
            Delete
          </Button>
        </div>
      </Td>
    </tr>
  );
}

export default function ActivityListTable<
  T extends ActivityListTable_period$key,
>({
  periods,
  firstcol,
  getRowLabel,
  hasNextPage,
  isLoadingMore,
  onLoadMore,
  loadMoreError,
}: {
  periods: ReadonlyArray<T>;
  firstcol: Firstcol;
  getRowLabel: (p: T) => string;
  hasNextPage?: boolean;
  isLoadingMore?: boolean;
  onLoadMore?: () => void;
  loadMoreError?: string | null;
}) {
  const { isDev, disaggregateVirtualPeriods } = useUserInfo();
  const [hideVirtual, setHideVirtual] = useState(false);
  const showSplit = disaggregateVirtualPeriods && !hideVirtual;
  const dayGroupedRows = new Map<string, Array<Entry<T>>>();
  const dateOptions: Intl.DateTimeFormatOptions = {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  };

  for (const periodRef of periods) {
    const data = readInlineData(activityListTablePeriod, periodRef);
    if (!data) continue;
    // Non-throwing: a failed category should keep the row visible (so its own
    // "unable to load" state can show), not hide it via a filter that can't
    // tell "not virtual" from "couldn't tell".
    if (
      hideVirtual &&
      isValueResult(data.category) &&
      data.category.value?.isVirtual
    ) {
      continue;
    }
    const startTime = new Date(data.startTime * 1000);
    const day = startTime.toLocaleDateString(undefined, dateOptions);
    if (!dayGroupedRows.has(day)) {
      dayGroupedRows.set(day, []);
    }
    dayGroupedRows.get(day)!.push({ ref: periodRef, data });
  }

  return (
    <>
      {disaggregateVirtualPeriods && (
        <label className="mb-3 flex items-center justify-end gap-2">
          <input
            type="checkbox"
            checked={hideVirtual}
            onChange={(e) => setHideVirtual(e.target.checked)}
          />
          Hide virtual
        </label>
      )}
      <AdminTable>
        <thead>
          <tr>
            <Th style={{ width: 20 }}></Th>
            {isDev && <Th>ID</Th>}
            <Th>{firstcol === "location" ? "Location" : "Name"}</Th>
            <Th>Start</Th>
            <Th>End</Th>
            <Th>Time</Th>
            <Th>Category</Th>
            <Th></Th>
          </tr>
        </thead>
        <tbody>
          {Array.from(dayGroupedRows).map(([day, entries]) => (
            <Section
              key={day}
              day={day}
              entries={entries}
              getRowLabel={getRowLabel}
              isDev={isDev}
              showSplit={showSplit}
            />
          ))}
        </tbody>
      </AdminTable>
      {hasNextPage && onLoadMore && (
        <p>
          <Button onClick={onLoadMore} disabled={isLoadingMore}>
            {isLoadingMore ? "Loading..." : "Load More"}
          </Button>
        </p>
      )}
      {loadMoreError && (
        <p className="font-bold text-red-600">{loadMoreError}</p>
      )}
    </>
  );
}
