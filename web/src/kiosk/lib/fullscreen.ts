/**
 * Fullscreen helpers for the kiosk. Safari only gained the unprefixed Fullscreen API in
 * iPadOS 16.4; iPadOS 12–16.3 has the `webkit`-prefixed one, so both are handled. iPhone
 * Safari has neither for anything but video, which `isFullscreenSupported` reports.
 */

type WebkitDocument = Document & {
  webkitFullscreenEnabled?: boolean;
  webkitFullscreenElement?: Element | null;
  webkitExitFullscreen?: () => Promise<void> | void;
};

type WebkitElement = HTMLElement & {
  webkitRequestFullscreen?: () => Promise<void> | void;
};

export function isFullscreenSupported(): boolean {
  const doc = document as WebkitDocument;
  return Boolean(doc.fullscreenEnabled || doc.webkitFullscreenEnabled);
}

export function isFullscreen(): boolean {
  const doc = document as WebkitDocument;
  return (doc.fullscreenElement ?? doc.webkitFullscreenElement ?? null) != null;
}

/** For `useSyncExternalStore`, with `isFullscreen` as the snapshot. */
export function subscribeFullscreen(listener: () => void): () => void {
  const events = ["fullscreenchange", "webkitfullscreenchange"];
  events.forEach((event) => document.addEventListener(event, listener));
  return () =>
    events.forEach((event) => document.removeEventListener(event, listener));
}

/** Must be called from a user gesture (a tap handler), or the browser refuses it. */
export async function toggleFullscreen(): Promise<void> {
  const doc = document as WebkitDocument;
  if (isFullscreen()) {
    if (typeof doc.exitFullscreen === "function") {
      await doc.exitFullscreen();
    } else {
      await doc.webkitExitFullscreen?.();
    }
    return;
  }
  const root = document.documentElement as WebkitElement;
  if (typeof root.requestFullscreen === "function") {
    await root.requestFullscreen();
  } else {
    await root.webkitRequestFullscreen?.();
  }
}

/**
 * True when running as a home-screen web app — already without any browser chrome, so
 * there is nothing for the Fullscreen API to add. `navigator.standalone` is iOS's own
 * flag, and predates its support for the `display-mode` media query.
 */
export function isStandalone(): boolean {
  const nav = navigator as Navigator & { standalone?: boolean };
  if (nav.standalone === true) {
    return true;
  }
  return (
    typeof window.matchMedia === "function" &&
    ["standalone", "fullscreen"].some(
      (mode) => window.matchMedia(`(display-mode: ${mode})`).matches,
    )
  );
}

/** iOS/iPadOS Safari, the only browser with `navigator.standalone`. */
export function isAppleMobileSafari(): boolean {
  return "standalone" in navigator;
}
