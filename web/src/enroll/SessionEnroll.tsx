import { graphql, useMutation } from "react-relay";
import { TextLink } from "../components/ui/TextLink";
import { useSearchParams } from "react-router";
import { Suspense, useState } from "react";
import SessionForm from "../admin/components/SessionForm";
import { NEW_SESSION_CONFIG } from "../admin/components/sessionConfig";
import { useUserInfo } from "../admin/components/useUserInfo";
import { useNotify } from "../admin/components/useNotify";
import { useRetryableLazyLoadQuery } from "../components/useRetryableLazyLoadQuery";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
} from "../components/ui/Panel";
import type {
  SessionEnrollQuery,
  SessionEnrollQuery$data,
} from "./__generated__/SessionEnrollQuery.graphql";
import type {
  KioskBindTarget,
  SessionEnrollBindMutation,
} from "./__generated__/SessionEnrollBindMutation.graphql";
import type { SessionEnrollKiosksQuery } from "./__generated__/SessionEnrollKiosksQuery.graphql";
import { FieldList, FormField } from "../components/ui/FormField";
import RelayErrorBoundary from "../components/RelayErrorBoundary";
import { OptionList, OptionRow } from "../components/ui/OptionList";
import Select from "../components/ui/Select";
import { Button } from "../components/ui/Button";
import LoadingIndicator from "../components/LoadingIndicator";
import { type KioskStatus, describeStatus, kioskStatus } from "./kioskStatus";

/**
 * Standalone page reached by scanning a kiosk's QR code (see EnrollApp.tsx for why it
 * lives outside the admin dashboard): **Set up this computer**. The `?fp=` query param
 * carries the device's public-key fingerprint.
 *
 * It says what the computer is now, then makes it a new kiosk or an existing one, both
 * through `bindKioskDevice` (plans/kiosk-key-binding.md). Choosing the kiosk the
 * computer already is brings it back after it expired; choosing another moves that kiosk
 * here, for a computer that lost its key. The kiosk the computer is now is only named
 * when it is at a location the admin can see: the QR code is not a way to look up
 * another unit's kiosks.
 */
export default function SessionEnroll() {
  const [searchParams] = useSearchParams();
  const fingerprint = searchParams.get("fp");

  if (!fingerprint) {
    return (
      <Notice title="Missing device fingerprint">
        This setup link is missing its device fingerprint. Rescan the QR code
        shown on the kiosk.
      </Notice>
    );
  }

  return <SessionEnrollForm fingerprint={fingerprint} />;
}

type Done = "new" | "moved" | "back";

type CurrentKiosk = NonNullable<
  NonNullable<SessionEnrollQuery$data["pendingEnrollmentKey"]>["currentKiosk"]
>;

function SessionEnrollForm({ fingerprint }: { fingerprint: string }) {
  const { notifyError } = useNotify();
  const { locations } = useUserInfo();
  const enabledLocations = locations
    .filter((loc) => loc.enabled)
    .sort((a, b) => a.name.localeCompare(b.name));
  const [done, setDone] = useState<Done | null>(null);
  // Bumped after a refused bind, to show the device and kiosks as they are now.
  const [fetchKey, setFetchKey] = useState(0);

  const data = useRetryableLazyLoadQuery<SessionEnrollQuery>(
    graphql`
      query SessionEnrollQuery($fingerprint: String!) @throwOnFieldError {
        pendingEnrollmentKey(fingerprint: $fingerprint) {
          version
          currentKiosk {
            id
            name
            keyExpiresAt
            location {
              id
              name
            }
          }
        }
      }
    `,
    { fingerprint },
    { fetchKey, fetchPolicy: "store-and-network" },
  );
  const pending = data.pendingEnrollmentKey;
  const current = pending?.currentKiosk ?? null;
  const [existing, setExisting] = useState(current != null);

  const [commitBind, isBindInFlight] = useMutation<SessionEnrollBindMutation>(
    graphql`
      mutation SessionEnrollBindMutation(
        $keyFingerprint: String!
        $expectedKeyVersion: Int!
        $target: KioskBindTarget!
      ) {
        bindKioskDevice(
          keyFingerprint: $keyFingerprint
          expectedKeyVersion: $expectedKeyVersion
          target: $target
        ) {
          id
          keyEnrolled
          keyExpiresAt
          keyReleasedAt
          reactivatable
        }
      }
    `,
  );

  /** Bind the device; false (having said why) if it was refused. */
  async function bind(
    target: KioskBindTarget,
    locationId: string,
  ): Promise<boolean> {
    if (pending == null) return false;
    try {
      await new Promise((resolve, reject) => {
        commitBind({
          variables: {
            keyFingerprint: fingerprint,
            expectedKeyVersion: pending.version,
            target,
          },
          onCompleted: resolve,
          onError: reject,
          // The location's kiosk list changed, and a kiosk elsewhere in it may have
          // given up this computer's key.
          updater: (store) => store.get(locationId)?.invalidateRecord(),
        });
      });
      return true;
    } catch (err) {
      notifyError(err, "Couldn't set up this computer");
      // What the admin chose from may be out of date: show it as it is now.
      setFetchKey((key) => key + 1);
      return false;
    }
  }

  async function handleNew(formData: FormData) {
    const locationId = formData.get("locationId")?.toString() || "";
    const ok = await bind(
      {
        new: {
          locationId,
          name: formData.get("name")?.toString() || "",
          config: formData.get("config")?.toString() || "",
          healthcheckUrl: formData.get("healthcheckUrl")?.toString() || "",
        },
      },
      locationId,
    );
    if (ok) setDone("new");
  }

  if (done) {
    return (
      <Notice
        title={
          done === "new"
            ? "Kiosk set up"
            : done === "back"
              ? "Kiosk back"
              : "Kiosk moved"
        }
      >
        The kiosk screen will switch over automatically within a few seconds
        {done === "moved" ? ", keeping its name and settings" : ""}. You can
        close this page, or head to{" "}
        <TextLink to="/admin/sessions">the kiosks list</TextLink>.
      </Notice>
    );
  }

  if (pending == null) {
    return (
      <Notice title="Setup request expired">
        This computer is no longer waiting to be set up. Check that the kiosk is
        still showing its QR code and rescan it.
      </Notice>
    );
  }

  if (enabledLocations.length === 0) {
    return (
      <Notice title="No locations available">
        No locations available to set this kiosk up at. Please contact an
        administrator.
      </Notice>
    );
  }

  return (
    <Panel>
      <PanelBox wide>
        <PanelTitle>Set up this computer</PanelTitle>
        <PanelIntro>
          <CurrentKioskSummary current={current} />
        </PanelIntro>

        <OptionList role="radiogroup" aria-label="New or existing kiosk">
          <OptionRow
            input={
              <input
                type="radio"
                name="existing"
                checked={!existing}
                onChange={() => setExisting(false)}
                className="mt-0.5"
              />
            }
            title="New kiosk"
            description="set this computer up as a kiosk of its own"
          />
          <OptionRow
            input={
              <input
                type="radio"
                name="existing"
                checked={existing}
                onChange={() => setExisting(true)}
                className="mt-0.5"
              />
            }
            title="An existing kiosk"
            description={
              current
                ? "bring this computer's kiosk back, or make it another kiosk that's already set up, keeping that kiosk's name and settings"
                : "this computer takes over a kiosk that's already set up, keeping its name and settings; use it when a kiosk that was working shows its QR code again"
            }
          />
        </OptionList>

        {existing ? (
          <ExistingKioskForm
            current={current}
            locations={enabledLocations}
            fetchKey={fetchKey}
            isBindInFlight={isBindInFlight}
            onBind={async (kiosk, locationId) => {
              const back = kiosk.id === current?.id;
              const ok = await bind(
                {
                  existing: { id: kiosk.id, expectedVersion: kiosk.version },
                },
                locationId,
              );
              if (ok) setDone(back ? "back" : "moved");
            }}
          />
        ) : (
          <>
            <p className="mt-4 mb-0">
              Choose the location it belongs to and give it a name to describe
              the location or type of computer, then save — the kiosk will start
              working automatically, no code required.
            </p>
            <SessionForm
              initialName=""
              initialConfig={NEW_SESSION_CONFIG}
              initialHealthcheckUrl=""
              locations={enabledLocations}
              showHealthcheckUrl={false}
              isMutationInFlight={isBindInFlight}
              onSubmit={handleNew}
            />
          </>
        )}
      </PanelBox>
    </Panel>
  );
}

/** What the computer is now, as far as this admin can see. */
function CurrentKioskSummary({ current }: { current: CurrentKiosk | null }) {
  const [now] = useState(() => Math.round(Date.now() / 1000));
  if (current == null) {
    return <>This computer isn't a kiosk at any of your units.</>;
  }
  const expired = current.keyExpiresAt != null && current.keyExpiresAt <= now;
  return (
    <>
      This computer is the kiosk <strong>{current.name}</strong> at{" "}
      <strong>{current.location.name}</strong>.{" "}
      {expired
        ? "It has expired. It's chosen below: confirm to bring it back."
        : "It's chosen below: confirm to bring it back, or set this computer up as another kiosk."}
    </>
  );
}

/** A kiosk this computer could become. */
type ExistingKiosk = {
  id: string;
  name: string;
  version: number;
  status: KioskStatus;
};

/** Pick a location, then one of its kiosks for this computer to be. */
function ExistingKioskForm({
  current,
  locations,
  fetchKey,
  isBindInFlight,
  onBind,
}: {
  current: CurrentKiosk | null;
  locations: ReadonlyArray<{ readonly id: string; readonly name: string }>;
  fetchKey: number;
  isBindInFlight: boolean;
  onBind: (kiosk: ExistingKiosk, locationId: string) => Promise<void>;
}) {
  const [locationId, setLocationId] = useState(current?.location.id ?? "");

  return (
    <FieldList>
      <FormField label={<label htmlFor="existingLocationId">Location</label>}>
        <Select
          id="existingLocationId"
          required
          value={locationId}
          onChange={(e) => setLocationId(e.target.value)}
        >
          <option value="" disabled>
            Select a location…
          </option>
          {locations.map((location) => (
            <option key={location.id} value={location.id}>
              {location.name}
            </option>
          ))}
        </Select>
      </FormField>
      {locationId && (
        <RelayErrorBoundary canRetry>
          <Suspense fallback={<LoadingIndicator />}>
            <KioskChoice
              // A new location starts with nothing chosen.
              key={locationId}
              locationId={locationId}
              currentId={current?.id ?? null}
              fetchKey={fetchKey}
              isBindInFlight={isBindInFlight}
              onBind={(kiosk) => onBind(kiosk, locationId)}
            />
          </Suspense>
        </RelayErrorBoundary>
      )}
    </FieldList>
  );
}

/** A location's kiosks, with the one chosen and the button that confirms it. */
function KioskChoice({
  locationId,
  currentId,
  fetchKey,
  isBindInFlight,
  onBind,
}: {
  locationId: string;
  currentId: string | null;
  fetchKey: number;
  isBindInFlight: boolean;
  onBind: (kiosk: ExistingKiosk) => Promise<void>;
}) {
  // Each session is handed to `kioskStatus` whole rather than read field by field, so
  // the lint rule can't see the usage.
  /* eslint-disable relay/unused-fields */
  const data = useRetryableLazyLoadQuery<SessionEnrollKiosksQuery>(
    graphql`
      query SessionEnrollKiosksQuery($location: ID!) @throwOnFieldError {
        location(id: $location) {
          viewerCanEdit
          sessions {
            id
            name
            lastContact
            version
            keyEnrolled
            keyReleasedAt
          }
        }
      }
    `,
    { location: locationId },
    // Always fresh: the statuses and the bind's version check go by what this shows.
    { fetchKey, fetchPolicy: "store-and-network" },
  );
  /* eslint-enable relay/unused-fields */
  // The computer's own kiosk, when it is at this location, starts chosen.
  const [selectedId, setSelectedId] = useState<string | null>(currentId);
  const [now] = useState(() => Math.round(Date.now() / 1000));
  const kiosks: ExistingKiosk[] = (
    data.location?.viewerCanEdit ? data.location.sessions : []
  ).map((session) => ({
    id: session.id,
    name: session.name,
    version: session.version,
    status: kioskStatus(session, currentId, now),
  }));
  // As loaded now, so a refetch after a refusal confirms against the fresh version.
  const kiosk = kiosks.find((k) => k.id === selectedId) ?? null;

  async function confirmAndBind() {
    if (kiosk == null) return;
    if (kiosk.status.kind !== "this") {
      const confirmed = confirm(
        `Move ${kiosk.name} to this computer? It keeps its name and settings. ` +
          (kiosk.status.kind === "online"
            ? `It's online on another computer, which will be disconnected and show its QR code.`
            : `If another computer is still using it, that one will be disconnected.`),
      );
      if (!confirmed) return;
    }
    await onBind(kiosk);
  }

  return (
    <>
      <FormField label={<span>Kiosk</span>}>
        {kiosks.length === 0 ? (
          <p className="m-0 text-ink-muted">
            No kiosks you can set up at this location.
          </p>
        ) : (
          <OptionList role="radiogroup" aria-label="Kiosk">
            {kiosks.map((k) => (
              <OptionRow
                key={k.id}
                input={
                  <input
                    type="radio"
                    name="sessionId"
                    value={k.id}
                    checked={selectedId === k.id}
                    onChange={() => setSelectedId(k.id)}
                    className="mt-0.5"
                  />
                }
                title={k.name}
                description={describeStatus(k.status)}
              />
            ))}
          </OptionList>
        )}
      </FormField>
      <FormField>
        <Button
          onClick={confirmAndBind}
          disabled={kiosk == null || isBindInFlight}
        >
          {kiosk?.status.kind === "this"
            ? "Bring kiosk back"
            : "Use this computer"}
        </Button>
      </FormField>
    </>
  );
}

function Notice({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <Panel>
      <PanelBox>
        <PanelTitle>{title}</PanelTitle>
        <PanelIntro>{children}</PanelIntro>
      </PanelBox>
    </Panel>
  );
}
