import { describe, it, expect } from "vitest";
import { buildSignedAuthHeader, keyInfoFromKeyPair } from "./kioskKey";

function base64ToBytes(b64: string): ArrayBuffer {
  const binary = atob(b64);
  const buf = new ArrayBuffer(binary.length);
  const bytes = new Uint8Array(buf);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return buf;
}

function enc(text: string): ArrayBuffer {
  return new TextEncoder().encode(text).buffer as ArrayBuffer;
}

async function sha256Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", enc(text));
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

describe("buildSignedAuthHeader", () => {
  it("produces a SLKey header whose signature verifies over the canonical payload", async () => {
    const keyPair = await crypto.subtle.generateKey(
      { name: "ECDSA", namedCurve: "P-256" },
      true,
      ["sign", "verify"],
    );
    const info = await keyInfoFromKeyPair(keyPair);
    const body = JSON.stringify({ query: "query { session { id } }" });

    const header = await buildSignedAuthHeader(info, body);

    const [scheme, rest] = header.split(" ");
    expect(scheme).toBe("SLKey");
    const [fp, ts, sig] = rest.split(".");
    expect(fp).toBe(info.fingerprint);
    expect(fp).toMatch(/^[0-9a-f]{64}$/);
    expect(ts).toMatch(/^\d+$/);

    // Reconstruct exactly what api/src/session_key.rs::canonical_payload builds and
    // confirm the signature verifies — this is the cross-language contract.
    const bodyHashHex = await sha256Hex(body);
    const payload = `slkey-v1:${fp}:${ts}:${bodyHashHex}`;
    const ok = await crypto.subtle.verify(
      { name: "ECDSA", hash: "SHA-256" },
      keyPair.publicKey,
      base64ToBytes(sig),
      enc(payload),
    );
    expect(ok).toBe(true);
  });

  it("binds the body: a signature does not verify against a different body", async () => {
    const keyPair = await crypto.subtle.generateKey(
      { name: "ECDSA", namedCurve: "P-256" },
      true,
      ["sign", "verify"],
    );
    const info = await keyInfoFromKeyPair(keyPair);
    const header = await buildSignedAuthHeader(info, "original body");

    const [, rest] = header.split(" ");
    const [fp, ts, sig] = rest.split(".");
    const tamperedHash = await sha256Hex("other body");
    const payload = `slkey-v1:${fp}:${ts}:${tamperedHash}`;
    const ok = await crypto.subtle.verify(
      { name: "ECDSA", hash: "SHA-256" },
      keyPair.publicKey,
      base64ToBytes(sig),
      enc(payload),
    );
    expect(ok).toBe(false);
  });
});
