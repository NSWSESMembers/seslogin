import { Fragment, Suspense, useState } from "react";
import { graphql, isValueResult, readInlineData } from "relay-runtime";
import { fetchQuery, useRelayEnvironment } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type {
  AuditLogQuery,
  AuditEntityType,
} from "./__generated__/AuditLogQuery.graphql";
import type {
  AuditLog_entry$data,
  AuditLog_entry$key,
} from "./__generated__/AuditLog_entry.graphql";
import useSelectedLocation from "../components/useSelectedLocation";
import { useUserInfo } from "../components/useUserInfo";
import {
  ENTITY_TYPE_LABELS,
  actionLabel,
  actorKindHint,
  actorText,
  entityTypeLabel,
  formatChangeValue,
  isRedacted,
} from "../components/auditLogFormat";
import { formatFullDateTime } from "../../lib/time";
import { cn } from "../../lib/tw";
import LoadingIndicator from "../../components/LoadingIndicator";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button } from "../../components/ui/Button";
import { Muted } from "../../components/ui/Muted";
import Select from "../../components/ui/Select";
import { StatusMessage } from "../../components/ui/StatusMessage";

const AUDIT_PAGE_SIZE = 50;

// Below the md breakpoint six columns don't fit a phone, so each entry becomes a
// small card instead: the header row goes and every cell is a labelled line.
const MOBILE_ROW =
  "max-md:block max-md:border-b max-md:border-line-faint max-md:px-2 max-md:py-1.5";
const MOBILE_CELL =
  "max-md:block max-md:border-b-0 max-md:px-0 max-md:py-0.5 max-md:before:mr-2 max-md:before:text-xs max-md:before:font-semibold max-md:before:text-ink-muted max-md:before:uppercase max-md:before:content-[attr(data-label)]";

// The data one table row shows. Both `Location.auditLog` and `Query.auditLog`
// return the same entry type, so one fragment serves both views. Marked @inline
// so the page can hold plain refs in state and read each one where it renders.
const auditLogEntry = graphql`
  fragment AuditLog_entry on AuditEntry @inline {
    id
    timestamp
    action
    entityType
    entityId
    entityLabel
    # @catch on the nullable relations/lookups: one dangling reference must
    # degrade that cell, not (via @throwOnFieldError on the query) the page.
    location @catch {
      id
      name
    }
    actor {
      kind
      # Aliased: Relay normalises any object with an id field under that id,
      # and AuditActor.id is a *user's* (or kiosk's) ID, so unaliased the actor
      # would overwrite that user's record in the store.
      actorId: id
      via
      label @catch
    }
    ip
    changes {
      field
      before
      after
    }
  }
`;

// One query for both views: a location's log, or (super users) the log across
// every location. Load more reuses it with a cursor.
const auditLogQuery = graphql`
  query AuditLogQuery(
    $location: ID!
    $all: Boolean!
    $first: Int!
    $after: String
    $entityType: AuditEntityType
  ) @throwOnFieldError {
    location(id: $location) @skip(if: $all) {
      id
      auditLog(first: $first, after: $after, entityType: $entityType) {
        edges {
          node {
            ...AuditLog_entry
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
    auditLog(first: $first, after: $after, entityType: $entityType)
      @include(if: $all) {
      edges {
        node {
          ...AuditLog_entry
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
`;

type Entry = AuditLog_entry$data;
type EntryRef = AuditLog_entry$key;
type Page = {
  readonly edges: ReadonlyArray<{ readonly node: EntryRef }>;
  readonly pageInfo: {
    readonly hasNextPage: boolean;
    readonly endCursor: string | null | undefined;
  };
};

function pageOf(data: AuditLogQuery["response"] | null | undefined): Page {
  return (
    data?.auditLog ??
    data?.location?.auditLog ?? {
      edges: [],
      pageInfo: { hasNextPage: false, endCursor: null },
    }
  );
}

export default function AuditLog() {
  const { isSuper } = useUserInfo();
  const location = useSelectedLocation();
  const [allLocations, setAllLocations] = useState(false);
  const [entityType, setEntityType] = useState<AuditEntityType | "">("");
  // Only super users can ask for the cross-location log; never trust stale state.
  const all = isSuper && allLocations;

  return (
    <>
      <p className="my-4">
        {all
          ? "Changes made in every location, and to records that belong to none, newest first."
          : `Changes made at ${location.name}, newest first.`}
      </p>
      <div className="mb-3 flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2">
          <span className="text-sm font-semibold text-ink-strong">Show</span>
          <Select
            width="auto"
            value={entityType}
            onChange={(e) =>
              setEntityType(e.target.value as AuditEntityType | "")
            }
          >
            <option value="">All changes</option>
            {Object.entries(ENTITY_TYPE_LABELS).map(([type, label]) => (
              <option key={type} value={type}>
                {label}
              </option>
            ))}
          </Select>
        </label>
        {isSuper && (
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={allLocations}
              onChange={(e) => setAllLocations(e.target.checked)}
            />
            All locations
          </label>
        )}
      </div>
      <Suspense fallback={<LoadingIndicator />}>
        {/* Keyed so any change of view starts again from the first page. */}
        <AuditLogContent
          key={`${all}|${location.id}|${entityType}`}
          all={all}
          locationId={location.id}
          entityType={entityType === "" ? null : entityType}
        />
      </Suspense>
    </>
  );
}

function AuditLogContent({
  all,
  locationId,
  entityType,
}: {
  all: boolean;
  locationId: string;
  entityType: AuditEntityType | null;
}) {
  const relayEnvironment = useRelayEnvironment();
  const variables = {
    location: locationId,
    all,
    first: AUDIT_PAGE_SIZE,
    entityType,
  };
  const data = useRetryableLazyLoadQuery<AuditLogQuery>(auditLogQuery, {
    ...variables,
    after: null,
  });

  // Pages after the first, appended as "Load more" fetches them. The first page
  // always comes straight from the query above.
  const [more, setMore] = useState<{
    entries: EntryRef[];
    hasNextPage: boolean;
    endCursor: string | null;
  } | null>(null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [loadMoreError, setLoadMoreError] = useState<string | null>(null);

  const first = pageOf(data);
  const refs = [
    ...first.edges.map((edge) => edge.node),
    ...(more?.entries ?? []),
  ];
  const hasNextPage = more ? more.hasNextPage : first.pageInfo.hasNextPage;
  const endCursor = more ? more.endCursor : (first.pageInfo.endCursor ?? null);

  async function onLoadMore() {
    if (!hasNextPage || !endCursor || isLoadingMore) return;
    setIsLoadingMore(true);
    setLoadMoreError(null);
    try {
      const next = await fetchQuery<AuditLogQuery>(
        relayEnvironment,
        auditLogQuery,
        { ...variables, after: endCursor },
      ).toPromise();
      const page = pageOf(next);
      setMore((previous) => ({
        entries: [
          ...(previous?.entries ?? []),
          ...page.edges.map((edge) => edge.node),
        ],
        hasNextPage: page.pageInfo.hasNextPage,
        endCursor: page.pageInfo.endCursor ?? null,
      }));
    } catch (err) {
      console.error("Failed to load more audit entries:", err);
      setLoadMoreError("Couldn't load more — please try again.");
    } finally {
      setIsLoadingMore(false);
    }
  }

  if (refs.length === 0) {
    return (
      <Muted className="my-4">
        {entityType
          ? `No ${entityTypeLabel(entityType)} changes have been recorded.`
          : "No changes have been recorded yet."}
      </Muted>
    );
  }

  return (
    <>
      <AuditTable refs={refs} showLocation={all} />
      {hasNextPage && (
        <p className="my-4">
          <Button onClick={onLoadMore} disabled={isLoadingMore}>
            {isLoadingMore ? "Loading..." : "Load More"}
          </Button>
        </p>
      )}
      {loadMoreError && (
        <StatusMessage variant="error">{loadMoreError}</StatusMessage>
      )}
    </>
  );
}

function AuditTable({
  refs,
  showLocation,
}: {
  refs: ReadonlyArray<EntryRef>;
  showLocation: boolean;
}) {
  const { isSuper } = useUserInfo();
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const colSpan = showLocation ? 6 : 5;

  function toggle(id: string) {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }

  return (
    <AdminTable>
      <thead className="max-md:hidden">
        <tr>
          <Th>When</Th>
          <Th>Who</Th>
          <Th>Action</Th>
          <Th>What</Th>
          {showLocation && <Th>Location</Th>}
          <Th></Th>
        </tr>
      </thead>
      <tbody>
        {refs.map((ref, idx) => {
          const entry = readInlineData<AuditLog_entry$key>(auditLogEntry, ref);
          const isOpen = expanded.has(entry.id);
          const stripe = idx % 2 === 0 ? "bg-surface-raised" : undefined;
          // The details row continues the summary row, so drop the rule between.
          const cell = cn(MOBILE_CELL, isOpen && "border-b-0");
          return (
            <Fragment key={entry.id}>
              <tr
                className={cn(
                  stripe,
                  MOBILE_ROW,
                  isOpen && "max-md:border-b-0",
                )}
              >
                <Td nowrap data-label="When" className={cell}>
                  {formatFullDateTime(new Date(entry.timestamp * 1000))}
                </Td>
                <Td data-label="Who" className={cell}>
                  <Who entry={entry} showVia={isSuper} />
                </Td>
                <Td data-label="Action" className={cell}>
                  {actionLabel(entry.action)}
                </Td>
                <Td data-label="What" className={cell}>
                  <span className="text-ink-muted">
                    {entityTypeLabel(entry.entityType)}
                  </span>{" "}
                  {entry.entityLabel ?? (
                    <span className="text-ink-muted">{entry.entityId}</span>
                  )}
                </Td>
                {showLocation && (
                  <Td data-label="Location" className={cell}>
                    <LocationName entry={entry} />
                  </Td>
                )}
                <Td
                  options
                  className={cn(
                    cell,
                    "max-md:w-auto max-md:pt-1 max-md:text-left",
                  )}
                >
                  <Button
                    size="row"
                    aria-expanded={isOpen}
                    onClick={() => toggle(entry.id)}
                  >
                    {isOpen ? "Hide details" : "Details"}
                  </Button>
                </Td>
              </tr>
              {isOpen && (
                <tr className={cn(stripe, MOBILE_ROW)}>
                  <Td
                    colSpan={colSpan}
                    className="max-md:block max-md:border-b-0 max-md:px-0"
                  >
                    <Details entry={entry} showIp={isSuper} />
                  </Td>
                </tr>
              )}
            </Fragment>
          );
        })}
      </tbody>
    </AdminTable>
  );
}

function Who({ entry, showVia }: { entry: Entry; showVia: boolean }) {
  const { actor } = entry;
  // A failed name lookup falls back to the kind-specific text, like a missing one.
  const label = isValueResult(actor.label) ? actor.label.value : null;
  const hint = label ? actorKindHint(actor.kind) : null;
  return (
    <>
      {actorText({ kind: actor.kind, id: actor.actorId, label })}
      {hint && <div className="text-xs text-ink-muted">{hint}</div>}
      {showVia && actor.via && (
        <div className="font-mono text-xs break-all text-ink-muted">
          via {actor.via}
        </div>
      )}
    </>
  );
}

function LocationName({ entry }: { entry: Entry }) {
  if (!isValueResult(entry.location)) {
    return <span className="text-ink-muted">Unknown</span>;
  }
  const location = entry.location.value;
  return location ? (
    <>{location.name}</>
  ) : (
    <span className="text-ink-muted">—</span>
  );
}

function ChangeValue({
  field,
  value,
}: {
  field: string;
  value: string | null | undefined;
}) {
  if (value == null || isRedacted(value)) {
    return (
      <span className="text-ink-muted">{formatChangeValue(field, value)}</span>
    );
  }
  return (
    <span className="wrap-break-word">{formatChangeValue(field, value)}</span>
  );
}

function Details({ entry, showIp }: { entry: Entry; showIp: boolean }) {
  return (
    <div className="py-1 text-sm">
      {entry.changes.length === 0 ? (
        <Muted>No field-level detail was recorded for this change.</Muted>
      ) : (
        <ul className="m-0 list-none space-y-1 p-0">
          {entry.changes.map((change) => (
            <li
              key={change.field}
              className="flex flex-wrap items-baseline gap-x-2"
            >
              <span className="font-mono font-semibold">{change.field}</span>
              <ChangeValue field={change.field} value={change.before} />
              <span className="text-ink-muted" aria-label="changed to">
                →
              </span>
              <ChangeValue field={change.field} value={change.after} />
            </li>
          ))}
        </ul>
      )}
      <Muted className="mt-2 text-xs break-all">
        {entityTypeLabel(entry.entityType)} ID {entry.entityId}
        {showIp && entry.ip ? ` · IP ${entry.ip}` : ""}
      </Muted>
    </div>
  );
}
