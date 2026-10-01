import useSelectedLocation from "./useSelectedLocation";

/**
 * Whether the current user may make changes at the selected location. False for a
 * Read only grant; the API enforces this too, so this only decides which controls
 * to offer.
 */
export default function useCanEditSelectedLocation() {
  return useSelectedLocation().viewerCanEdit;
}
