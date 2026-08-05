import { useEffect } from "react";

// The scan screen keeps the member ID input focused so barcode scans always land
// there, refocusing it shortly after it loses focus. Any UI that opens on top of
// the scan screen (the guest dialog, the category/adjust screens and their
// modals) takes a lease here for as long as it is up, which suspends that
// refocusing — otherwise it steals focus mid-typing. The scan input takes focus
// back on its own once the last lease is released.

const holders = new Set<string>();
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) {
    listener();
  }
}

export function isScanFocusSuspended(): boolean {
  return holders.size > 0;
}

export function suspendScanFocus(id: string): void {
  if (holders.has(id)) {
    return;
  }
  holders.add(id);
  emit();
}

export function resumeScanFocus(id: string): void {
  if (!holders.delete(id)) {
    return;
  }
  emit();
}

export function onScanFocusSuspendedChange(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Hold a scan-focus lease under `id` while `active` (and the calling component
 * is mounted), suspending the scan input's automatic refocusing.
 */
export function useSuspendScanFocus(id: string, active = true): void {
  useEffect(() => {
    if (!active) {
      return;
    }
    suspendScanFocus(id);
    return () => {
      resumeScanFocus(id);
    };
  }, [id, active]);
}
