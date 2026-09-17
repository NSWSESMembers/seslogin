import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import type { RefObject } from "react";
import { useAnchoredPopup } from "./useAnchoredPopup";

/**
 * `Popover.test.tsx` and `Combobox.test.tsx` exercise this hook's dismiss and
 * viewport behaviour indirectly, through their own components — but jsdom
 * zeroes every `getBoundingClientRect`, so neither can see *when* a
 * re-measure happens, only that the popup opens and closes. This file pins
 * that piece directly: the anchor resizing on its own while the popup stays
 * open (a `MultiCombobox` token box growing a second line of pills), which
 * none of the scroll/resize/visualViewport listeners catch.
 */

class FakeResizeObserver {
  static instances: FakeResizeObserver[] = [];
  observed: Element[] = [];
  disconnected = false;
  callback: ResizeObserverCallback;
  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    FakeResizeObserver.instances.push(this);
  }
  observe(target: Element) {
    this.observed.push(target);
  }
  unobserve(target: Element) {
    this.observed = this.observed.filter((t) => t !== target);
  }
  disconnect() {
    this.disconnected = true;
  }
}

function ref<T extends HTMLElement>(el: T): RefObject<T | null> {
  return { current: el };
}

let originalResizeObserver: typeof ResizeObserver | undefined;

beforeEach(() => {
  FakeResizeObserver.instances = [];
  originalResizeObserver = globalThis.ResizeObserver;
  globalThis.ResizeObserver =
    FakeResizeObserver as unknown as typeof ResizeObserver;
});

afterEach(() => {
  globalThis.ResizeObserver = originalResizeObserver!;
});

test("re-measures when the anchor resizes on its own", () => {
  const anchor = document.createElement("div");
  const popup = document.createElement("ul");
  document.body.append(anchor, popup);
  const measureSpy = vi.spyOn(anchor, "getBoundingClientRect");

  // Refs are created once, outside the render callback: a fresh object on
  // every render (as a real `useRef` never produces) would put a
  // changed-every-time value in the effect's dependency array and the
  // resulting re-measure loop would never settle.
  const anchorRef = ref(anchor);
  const popupRef = ref(popup);
  renderHook(() =>
    useAnchoredPopup({ open: true, onClose: () => {}, anchorRef, popupRef }),
  );

  const callsAfterOpen = measureSpy.mock.calls.length;
  expect(callsAfterOpen).toBeGreaterThan(0);

  const observer = FakeResizeObserver.instances.at(-1)!;
  expect(observer.observed).toEqual([anchor]);

  // The anchor growing a second line of pills, with no scroll or window
  // resize involved — the case nothing else here catches.
  observer.callback([], observer as unknown as ResizeObserver);

  expect(measureSpy.mock.calls.length).toBeGreaterThan(callsAfterOpen);

  anchor.remove();
  popup.remove();
});

test("disconnects the observer once the popup closes", () => {
  const anchor = document.createElement("div");
  const popup = document.createElement("ul");
  document.body.append(anchor, popup);

  const anchorRef = ref(anchor);
  const popupRef = ref(popup);
  const { rerender } = renderHook(
    ({ open }) =>
      useAnchoredPopup({ open, onClose: () => {}, anchorRef, popupRef }),
    { initialProps: { open: true } },
  );

  const observer = FakeResizeObserver.instances.at(-1)!;
  expect(observer.disconnected).toBe(false);

  rerender({ open: false });

  expect(observer.disconnected).toBe(true);

  anchor.remove();
  popup.remove();
});

test("does nothing when ResizeObserver isn't available", () => {
  globalThis.ResizeObserver = undefined as unknown as typeof ResizeObserver;
  const anchor = document.createElement("div");
  const popup = document.createElement("ul");
  document.body.append(anchor, popup);

  const anchorRef = ref(anchor);
  const popupRef = ref(popup);
  expect(() =>
    renderHook(() =>
      useAnchoredPopup({ open: true, onClose: () => {}, anchorRef, popupRef }),
    ),
  ).not.toThrow();

  anchor.remove();
  popup.remove();
});
