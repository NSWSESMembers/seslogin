import Combobox from "../../components/ui/Combobox";
import Select from "../../components/ui/Select";
import { Button } from "../../components/ui/Button";
import type {
  LocationOption,
  LocationRole,
  LocationRoleEntry,
} from "./locationRoles";

/**
 * Per-location role editor for a user: one row per granted location with a
 * role select and a Remove button, plus a search box to add another location
 * (new rows default to Admin). Controlled; the parent owns the entries.
 */
export default function LocationRolesField(props: {
  id: string;
  locations: readonly LocationOption[];
  value: readonly LocationRoleEntry[];
  onChange: (value: LocationRoleEntry[]) => void;
}) {
  const { id, locations, value, onChange } = props;
  const names = new Map(locations.map((l) => [l.id, l.name]));
  const nameOf = (locationId: string) => names.get(locationId) ?? locationId;
  const taken = new Set(value.map((e) => e.locationId));
  const addable = locations
    .filter((l) => !taken.has(l.id))
    .map((l) => ({ value: l.id, label: l.name }));

  return (
    <div className="flex flex-col gap-2">
      {value.length > 0 && (
        <ul className="flex flex-col gap-1" aria-label="Location roles">
          {value.map((entry) => (
            <li key={entry.locationId} className="flex items-center gap-2">
              <span className="min-w-0 flex-1 truncate">
                {nameOf(entry.locationId)}
              </span>
              <Select
                width="auto"
                aria-label={`Role at ${nameOf(entry.locationId)}`}
                value={entry.role}
                onChange={(e) =>
                  onChange(
                    value.map((v) =>
                      v.locationId === entry.locationId
                        ? { ...v, role: e.target.value as LocationRole }
                        : v,
                    ),
                  )
                }
              >
                <option value="admin">Admin</option>
                <option value="readOnly">Read only</option>
              </Select>
              <Button
                size="row"
                variant="danger"
                aria-label={`Remove ${nameOf(entry.locationId)}`}
                onClick={() =>
                  onChange(
                    value.filter((v) => v.locationId !== entry.locationId),
                  )
                }
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
      <Combobox
        id={id}
        aria-label="Add location"
        value=""
        options={addable}
        placeholder="Add a location…"
        emptyText="No locations match"
        allowClear={false}
        onChange={(locationId) => {
          if (locationId) {
            onChange([...value, { locationId, role: "admin" }]);
          }
        }}
      />
    </div>
  );
}
