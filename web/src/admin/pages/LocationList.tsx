import { useState } from "react";
import {
  graphql,
  useFragment,
  useLazyLoadQuery,
  useMutation,
} from "react-relay";
import type { LocationList_item$key } from "./__generated__/LocationList_item.graphql";
import type { LocationListQuery } from "./__generated__/LocationListQuery.graphql";
import { useNavigate } from "react-router";
import type { LocationListToggleMutation } from "./__generated__/LocationListToggleMutation.graphql";
import type { LocationListEnqueueSyncMutation } from "./__generated__/LocationListEnqueueSyncMutation.graphql";
import { useSettingsDispatch } from "../../lib/settings";
import { formatFullDateTime } from "../../lib/time";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";

function Row(props: {
  location: LocationList_item$key;
  idx: number;
  isDev: boolean;
}) {
  const idx = props.idx;
  const isDev = props.isDev;
  const settingsDispatch = useSettingsDispatch()!;
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();
  const location = useFragment<LocationList_item$key>(
    graphql`
      fragment LocationList_item on Location {
        id
        name
        enabled
        nitcEnabled
        lastSuccessfulMemberSync
      }
    `,
    props.location,
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<LocationListToggleMutation>(graphql`
      mutation LocationListToggleMutation(
        $id: ID!
        $name: String!
        $enabled: Boolean!
        $nitcEnabled: Int
      ) {
        updateLocation(
          id: $id
          name: $name
          enabled: $enabled
          nitcEnabled: $nitcEnabled
        ) {
          id
          name
          enabled
          nitcEnabled
        }
      }
    `);

  const [commitSync, isSyncInFlight] =
    useMutation<LocationListEnqueueSyncMutation>(graphql`
      mutation LocationListEnqueueSyncMutation($locationId: ID!) {
        enqueueMemberSync(locationId: $locationId)
      }
    `);

  function triggerSync() {
    commitSync({
      variables: { locationId: location.id },
      onCompleted: () => {
        notifySuccess(`Sync queued for ${location.name}`);
      },
      onError: (err) => {
        notifyError(err, `Couldn't queue sync for ${location.name}`);
      },
    });
  }

  async function toggleEnabled() {
    const action = location.enabled ? "disable" : "enable";
    const yes = confirm(
      `Are you sure you want to ${action} location ${location.name}?`,
    );
    if (yes) {
      try {
        await new Promise((resolve, reject) => {
          commitMutation({
            variables: {
              id: location.id,
              name: location.name,
              nitcEnabled: location.nitcEnabled,
              enabled: !location.enabled,
            },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.invalidateStore();
            },
          });
        });
        notifySuccess(`Location ${location.name} ${action}d`);
      } catch (err) {
        notifyError(err, `Couldn't ${action} location ${location.name}`);
      }
    }
  }

  function switchToLocation() {
    settingsDispatch({
      type: "set_location",
      id: location.id,
    });
    navigate("/admin");
  }

  const lastSync = location.lastSuccessfulMemberSync
    ? formatFullDateTime(new Date(location.lastSuccessfulMemberSync * 1000))
    : "Never";

  return (
    <tr className={idx % 2 === 0 ? "bg-neutral-50" : undefined}>
      {isDev && <Td className="font-mono text-[0.85em]">{location.id}</Td>}
      <Td nowrap>
        <div className={location.enabled ? undefined : "line-through"}>
          {location.name}
        </div>
      </Td>
      <Td nowrap>{lastSync}</Td>
      <Td>
        {location.nitcEnabled
          ? new Date(location.nitcEnabled * 1000).toISOString().slice(0, 10)
          : ""}
      </Td>
      <Td options>
        <div className="flex justify-end gap-1">
          <Button size="row" onClick={switchToLocation}>
            Switch to
          </Button>
          <Button size="row" onClick={triggerSync} disabled={isSyncInFlight}>
            Sync
          </Button>
          <ButtonLink size="row" to={`/admin/locations/${location.id}`}>
            Edit
          </ButtonLink>
          <Button
            size="row"
            variant={location.enabled ? "danger" : "primary"}
            onClick={toggleEnabled}
            disabled={isMutationInFlight}
          >
            {location.enabled ? "Disable" : "Enable"}
          </Button>
        </div>
      </Td>
    </tr>
  );
}

export default function LocationList() {
  const { isDev } = useUserInfo();
  const [showDisabled, setShowDisabled] = useState(false);
  const data = useLazyLoadQuery<LocationListQuery>(
    graphql`
      query LocationListQuery {
        locations {
          id
          name
          enabled
          ...LocationList_item
        }
      }
    `,
    {},
  );

  const locations = data?.locations
    ?.filter((location) => location != null)
    .filter((location) => showDisabled || location.enabled)
    .sort((a, b) => {
      const aName = a.name ?? "";
      const bName = b.name ?? "";
      return aName.localeCompare(bName);
    });

  return (
    <>
      <p>
        <label>
          <input
            type="checkbox"
            checked={showDisabled}
            onChange={(e) => setShowDisabled(e.target.checked)}
          />{" "}
          Show disabled
        </label>
      </p>
      <AdminTable>
        <thead>
          <tr>
            {isDev && <Th>ID</Th>}
            <Th>Name</Th>
            <Th>Last Member Sync</Th>
            <Th>NITC Export</Th>
            <Th style={{ width: 100 }}></Th>
          </tr>
        </thead>
        <tbody>
          {locations?.map((location, idx) => (
            <Row
              key={location.id}
              location={location}
              idx={idx}
              isDev={isDev}
            />
          ))}
        </tbody>
      </AdminTable>
    </>
  );
}
