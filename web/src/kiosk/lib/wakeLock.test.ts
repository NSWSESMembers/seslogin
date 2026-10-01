import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  getWakeLockStatus,
  startWakeLock,
  subscribeWakeLock,
} from "./wakeLock";

class FakeSentinel extends EventTarget {
  released = false;
  release = vi.fn(() => {
    this.released = true;
    this.dispatchEvent(new Event("release"));
    return Promise.resolve();
  });
}

let visibility: DocumentVisibilityState = "visible";
let sentinels: FakeSentinel[] = [];
const requestLock = vi.fn(() => {
  const sentinel = new FakeSentinel();
  sentinels.push(sentinel);
  return Promise.resolve(sentinel);
});

function setVisibility(state: DocumentVisibilityState) {
  visibility = state;
  document.dispatchEvent(new Event("visibilitychange"));
}

async function settle() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("startWakeLock", () => {
  let stop: () => void = () => {};

  beforeEach(() => {
    visibility = "visible";
    sentinels = [];
    requestLock.mockClear();
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => visibility,
    });
    Object.defineProperty(navigator, "wakeLock", {
      configurable: true,
      value: { request: requestLock },
    });
  });

  afterEach(() => {
    stop();
    Reflect.deleteProperty(navigator, "wakeLock");
    Reflect.deleteProperty(document, "visibilityState");
  });

  it("holds the lock once started", async () => {
    stop = startWakeLock();
    await settle();
    expect(requestLock).toHaveBeenCalledWith("screen");
    expect(getWakeLockStatus().state).toBe("held");
  });

  it("re-acquires after the browser releases it on hide", async () => {
    stop = startWakeLock();
    await settle();
    setVisibility("hidden");
    await sentinels[0].release();
    expect(getWakeLockStatus().state).toBe("released");
    expect(requestLock).toHaveBeenCalledTimes(1);

    setVisibility("visible");
    await settle();
    expect(requestLock).toHaveBeenCalledTimes(2);
    expect(getWakeLockStatus().state).toBe("held");
  });

  it("retries a refused request on the next tap", async () => {
    requestLock.mockRejectedValueOnce(new Error("NotAllowedError"));
    stop = startWakeLock();
    await settle();
    expect(getWakeLockStatus()).toEqual({
      state: "released",
      lastError: "NotAllowedError",
    });

    document.dispatchEvent(new Event("pointerdown"));
    await settle();
    expect(requestLock).toHaveBeenCalledTimes(2);
    expect(getWakeLockStatus().state).toBe("held");
  });

  it("does not request again while held", async () => {
    stop = startWakeLock();
    await settle();
    document.dispatchEvent(new Event("pointerdown"));
    await settle();
    expect(requestLock).toHaveBeenCalledTimes(1);
  });

  it("releases the lock and stops listening when stopped", async () => {
    stop = startWakeLock();
    await settle();
    stop();
    expect(sentinels[0].released).toBe(true);
    document.dispatchEvent(new Event("pointerdown"));
    await settle();
    expect(requestLock).toHaveBeenCalledTimes(1);
  });

  it("notifies subscribers of each change", async () => {
    const listener = vi.fn();
    const unsubscribe = subscribeWakeLock(listener);
    stop = startWakeLock();
    await settle();
    // pending, then held
    expect(listener).toHaveBeenCalledTimes(2);
    unsubscribe();
    stop();
    expect(listener).toHaveBeenCalledTimes(2);
  });

  it("reports unsupported without the API", () => {
    Reflect.deleteProperty(navigator, "wakeLock");
    stop = startWakeLock();
    expect(getWakeLockStatus().state).toBe("unsupported");
  });
});
