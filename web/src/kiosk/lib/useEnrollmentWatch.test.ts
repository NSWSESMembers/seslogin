// @vitest-environment jsdom

import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { KioskKeyInfo } from "./kioskKey";
import type {
  RealtimeChannelStateChange,
  RealtimeClient,
  RealtimeConnectionStateChange,
  RealtimeMessage,
} from "./realtimeClient";

vi.mock("./realtimeClient", () => ({
  createRealtimeClient: vi.fn(),
}));

vi.mock("./enrollmentKey", () => ({
  EVENT_ENROLLMENT_COMPLETED: "enrollment.completed",
  fetchKeySessionId: vi.fn(),
  fetchEnrollmentRealtimeToken: vi.fn(),
}));

import { createRealtimeClient } from "./realtimeClient";
import {
  fetchEnrollmentRealtimeToken,
  fetchKeySessionId,
} from "./enrollmentKey";
import { useEnrollmentWatch } from "./useEnrollmentWatch";

const info = {
  keyPair: {} as CryptoKeyPair,
  publicKeySpkiB64: "cHVia2V5",
  fingerprint: "abc123",
} as KioskKeyInfo;

const token = {
  channel: "kiosk-enroll:seslogin_test:abc123",
  tokenRequest: {
    keyName: "k",
    ttl: 1,
    capability: "{}",
    clientId: "enroll:abc123",
    timestamp: 1,
    nonce: "n",
    mac: "m",
  },
};

function makeFakeClient() {
  let onMessage: (m: RealtimeMessage) => void = () => {};
  let onChannel: (c: RealtimeChannelStateChange) => void = () => {};
  let onConnection: (c: RealtimeConnectionStateChange) => void = () => {};
  const client: RealtimeClient = {
    subscribe: (l) => {
      onMessage = l;
    },
    onChannelStateChange: (l) => {
      onChannel = l;
    },
    onConnectionStateChange: (l) => {
      onConnection = l;
    },
    close: vi.fn(),
  };
  return {
    client,
    message: (name: string) => onMessage({ name, data: {} }),
    channel: (current: string, resumed = false) =>
      onChannel({ current, resumed }),
    connection: (current: string) => onConnection({ current }),
  };
}

/** Lets pending promise continuations run, without advancing fake timers. */
async function flush() {
  await vi.advanceTimersByTimeAsync(0);
}

function mount(onEnrolled = vi.fn(), enabled = true) {
  const hook = renderHook(() =>
    useEnrollmentWatch({ info, enabled, onEnrolled }),
  );
  return { ...hook, onEnrolled };
}

describe("useEnrollmentWatch", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(fetchKeySessionId).mockReset().mockResolvedValue(null);
    vi.mocked(fetchEnrollmentRealtimeToken).mockReset();
    vi.mocked(createRealtimeClient).mockReset();
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("does nothing while disabled", async () => {
    mount(vi.fn(), false);
    await flush();
    expect(fetchKeySessionId).not.toHaveBeenCalled();
    expect(fetchEnrollmentRealtimeToken).not.toHaveBeenCalled();
  });

  it("enrols immediately when the key is already enrolled", async () => {
    vi.mocked(fetchKeySessionId).mockResolvedValue("s1");
    const { onEnrolled } = mount();
    await flush();
    expect(onEnrolled).toHaveBeenCalledOnce();
    expect(fetchEnrollmentRealtimeToken).not.toHaveBeenCalled();
  });

  it("polls when realtime is disabled server-side", async () => {
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(null);
    const { onEnrolled } = mount();
    await flush();
    expect(createRealtimeClient).not.toHaveBeenCalled();
    const initial = vi.mocked(fetchKeySessionId).mock.calls.length;

    await vi.advanceTimersByTimeAsync(5_000);
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBeGreaterThan(
      initial,
    );
    expect(onEnrolled).not.toHaveBeenCalled();

    vi.mocked(fetchKeySessionId).mockResolvedValue("s1");
    await vi.advanceTimersByTimeAsync(5_000);
    expect(onEnrolled).toHaveBeenCalledOnce();
  });

  it("polls when the token request throws", async () => {
    vi.mocked(fetchEnrollmentRealtimeToken).mockRejectedValue(new Error("x"));
    mount();
    await flush();
    const initial = vi.mocked(fetchKeySessionId).mock.calls.length;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBeGreaterThan(
      initial,
    );
  });

  it("confirms a pushed message and calls onEnrolled once", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    const { onEnrolled } = mount();
    await flush();
    expect(createRealtimeClient).toHaveBeenCalledWith(
      expect.objectContaining({ channel: token.channel }),
    );

    vi.mocked(fetchKeySessionId).mockResolvedValue("s1");
    fake.message("enrollment.completed");
    fake.message("enrollment.completed");
    await flush();
    expect(onEnrolled).toHaveBeenCalledOnce();
  });

  it("ignores unrelated messages", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    mount();
    await flush();
    const before = vi.mocked(fetchKeySessionId).mock.calls.length;
    fake.message("something.else");
    await flush();
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBe(before);
  });

  it("retries the confirm while the server catches up", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    const { onEnrolled } = mount();
    await flush();

    vi.mocked(fetchKeySessionId)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValue("s1");
    fake.message("enrollment.completed");
    await flush();
    expect(onEnrolled).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(500);
    expect(onEnrolled).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(onEnrolled).toHaveBeenCalledOnce();
  });

  it("upgrades an in-flight attach check when a push lands mid-check", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    const { onEnrolled } = mount();
    await flush();

    // The attach check is still awaiting its response when the push arrives, and
    // that response is a refusal (the GSI hasn't caught up yet).
    let resolveAttachCheck: (id: string | null) => void = () => {};
    vi.mocked(fetchKeySessionId)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveAttachCheck = resolve;
          }),
      )
      .mockResolvedValue("s1");
    fake.channel("attached", false);
    fake.message("enrollment.completed");
    resolveAttachCheck(null);
    await flush();
    expect(onEnrolled).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(500);
    expect(onEnrolled).toHaveBeenCalledOnce();
  });

  it("checks once on a non-resumed attach", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    const { onEnrolled } = mount();
    await flush();
    const before = vi.mocked(fetchKeySessionId).mock.calls.length;

    vi.mocked(fetchKeySessionId).mockResolvedValue("s1");
    fake.channel("attached", false);
    await flush();
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBe(before + 1);
    expect(onEnrolled).toHaveBeenCalledOnce();
  });

  it("does not poll or check again on a resumed attach", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    mount();
    await flush();
    const before = vi.mocked(fetchKeySessionId).mock.calls.length;
    fake.channel("attached", true);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBe(before);
  });

  it("falls back to polling when the channel fails, and stops once reattached", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    mount();
    await flush();
    fake.channel("attached", true);

    fake.channel("failed");
    await flush();
    const afterFail = vi.mocked(fetchKeySessionId).mock.calls.length;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBeGreaterThan(
      afterFail,
    );

    fake.channel("attached", true);
    const afterAttach = vi.mocked(fetchKeySessionId).mock.calls.length;
    await vi.advanceTimersByTimeAsync(60_000);
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBe(afterAttach);
  });

  it("falls back to polling when the connection fails", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    mount();
    await flush();
    fake.connection("failed");
    await flush();
    const before = vi.mocked(fetchKeySessionId).mock.calls.length;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBeGreaterThan(
      before,
    );
  });

  it("falls back to polling when the client cannot be created", async () => {
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockRejectedValue(new Error("no ably"));
    mount();
    await flush();
    const before = vi.mocked(fetchKeySessionId).mock.calls.length;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBeGreaterThan(
      before,
    );
  });

  it("closes the client and stops timers on cleanup", async () => {
    const fake = makeFakeClient();
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    const { unmount } = mount();
    await flush();
    fake.channel("failed");
    await flush();
    unmount();
    expect(fake.client.close).toHaveBeenCalledOnce();

    const before = vi.mocked(fetchKeySessionId).mock.calls.length;
    await vi.advanceTimersByTimeAsync(60_000);
    expect(vi.mocked(fetchKeySessionId).mock.calls.length).toBe(before);
  });

  it("closes a client that finishes connecting after cleanup", async () => {
    const fake = makeFakeClient();
    let resolveClient: (c: RealtimeClient) => void = () => {};
    vi.mocked(fetchEnrollmentRealtimeToken).mockResolvedValue(token);
    vi.mocked(createRealtimeClient).mockReturnValue(
      new Promise((resolve) => {
        resolveClient = resolve;
      }),
    );
    const { unmount } = mount();
    await flush();
    unmount();
    resolveClient(fake.client);
    await flush();
    expect(fake.client.close).toHaveBeenCalledOnce();
  });
});
