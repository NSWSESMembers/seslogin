import type { CSSProperties } from "react";
import { formatTime, formatTimeDiff } from "../../lib/time";
import { graphql, readInlineData } from "relay-runtime";
import { useMutation } from "react-relay";
import type { ActivityListTableDeleteMutation } from "./__generated__/ActivityListTableDeleteMutation.graphql";
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
    nitcExportStatus
    nitcEventId
    signedInSession {
      id
      name
    }
    signedOutSession {
      id
      name
    }
    category {
      id
      name
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
}: {
  day: string;
  entries: ReadonlyArray<Entry<T>>;
  getRowLabel: (p: T) => string;
  isDev: boolean;
}) {
  const colSpan = isDev ? 8 : 7;
  const periodCount = entries.length;
  const uniqueMemberCount = new Set(
    entries.map((entry) => entry.data.personId).filter(Boolean),
  ).size;
  const periodLabel = periodCount === 1 ? "period" : "periods";
  const memberLabel = uniqueMemberCount === 1 ? "member" : "members";

  return (
    <>
      <tr>
        <Th section colSpan={colSpan}>
          {day} ({periodCount} {periodLabel}, {uniqueMemberCount} unique{" "}
          {memberLabel})
        </Th>
      </tr>
      <tr>
        <td className="h-0.75" colSpan={colSpan}></td>
      </tr>
      {entries.map((entry, idx) => (
        <Row
          key={entry.data.id}
          entry={entry}
          idx={idx}
          getRowLabel={getRowLabel}
          isDev={isDev}
        />
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
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] =
    useMutation<ActivityListTableDeleteMutation>(graphql`
      mutation ActivityListTableDeleteMutation($id: ID!) {
        deletePeriod(id: $id)
      }
    `);

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
    <tr className={idx % 2 === 0 ? "bg-neutral-50" : undefined}>
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
        title={period.signedInSession?.name ?? undefined}
        style={period.signedInSession ? sessionHintStyle : undefined}
      >
        {formatTime(start)}
      </Td>
      <Td
        title={period.signedOutSession?.name ?? undefined}
        style={period.signedOutSession ? sessionHintStyle : undefined}
      >
        {end ? formatTime(end) : ""}
      </Td>
      <Td>{timeDiff}</Td>
      <Td>{period.category?.name}</Td>
      <Td options>
        <div className="flex justify-end gap-1">
          <ButtonLink size="row" to={`/admin/activity/${period.id}`}>
            Edit
          </ButtonLink>
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
}: {
  periods: ReadonlyArray<T>;
  firstcol: Firstcol;
  getRowLabel: (p: T) => string;
  hasNextPage?: boolean;
  isLoadingMore?: boolean;
  onLoadMore?: () => void;
}) {
  const { isDev } = useUserInfo();
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
    const startTime = new Date(data.startTime * 1000);
    const day = startTime.toLocaleDateString(undefined, dateOptions);
    if (!dayGroupedRows.has(day)) {
      dayGroupedRows.set(day, []);
    }
    dayGroupedRows.get(day)!.push({ ref: periodRef, data });
  }

  return (
    <>
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
    </>
  );
}
