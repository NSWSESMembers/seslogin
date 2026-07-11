/**
 * Non-exportable kiosk signing key management for the public-key enrollment flow.
 *
 * A kiosk generates an ECDSA P-256 keypair whose private key is non-extractable and
 * lives only in IndexedDB (CryptoKeys are structured-cloneable, so they persist without
 * ever exposing the private key material). The public key is published for enrollment,
 * and every authenticated request is signed with the private key.
 *
 * The signed payload and encodings here must match `api/src/session_key.rs` exactly:
 *   - canonical payload: `slkey-v1:<fingerprint>:<timestamp>:<bodyHashHex>`
 *   - fingerprint: hex SHA-256 of the SPKI DER
 *   - public key / signature: standard (not URL-safe) base64
 *   - signature: raw 64-byte r||s (what WebCrypto ECDSA produces)
 */

const DB_NAME = "seslogin-kiosk";
const STORE_NAME = "keys";
const PAYLOAD_VERSION = "slkey-v1";

export type KioskKeyInfo = {
  keyPair: CryptoKeyPair;
  publicKeySpkiB64: string;
  fingerprint: string;
};

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => {
      req.result.createObjectStore(STORE_NAME);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function idbGet(db: IDBDatabase, key: string): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readonly");
    const req = tx.objectStore(STORE_NAME).get(key);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function idbPut(db: IDBDatabase, key: string, value: unknown): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readwrite");
    tx.objectStore(STORE_NAME).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

function bufToHex(buf: ArrayBuffer): string {
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function bufToBase64(buf: ArrayBuffer): string {
  let binary = "";
  const bytes = new Uint8Array(buf);
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

async function sha256Hex(bytes: BufferSource): Promise<string> {
  return bufToHex(await crypto.subtle.digest("SHA-256", bytes));
}

/** Derive the public-key SPKI/base64 and fingerprint for an existing keypair. */
export async function keyInfoFromKeyPair(
  keyPair: CryptoKeyPair,
): Promise<KioskKeyInfo> {
  const spki = await crypto.subtle.exportKey("spki", keyPair.publicKey);
  return {
    keyPair,
    publicKeySpkiB64: bufToBase64(spki),
    fingerprint: await sha256Hex(spki),
  };
}

/**
 * Load the kiosk's persisted keypair for the given profile, generating and storing a
 * fresh non-extractable one if none exists. The key is never rotated by callers — a
 * disabled session reuses the same key to re-enroll.
 */
export async function getOrCreateKioskKey(
  profile: string,
): Promise<KioskKeyInfo> {
  const db = await openDb();
  const existing = (await idbGet(db, profile)) as CryptoKeyPair | undefined;
  if (existing?.privateKey && existing?.publicKey) {
    return keyInfoFromKeyPair(existing);
  }

  const keyPair = await crypto.subtle.generateKey(
    { name: "ECDSA", namedCurve: "P-256" },
    // Non-extractable: the private key can be used to sign but never exported. (The
    // public key is always exportable regardless of this flag.)
    false,
    ["sign", "verify"],
  );
  await idbPut(db, profile, keyPair);
  return keyInfoFromKeyPair(keyPair);
}

/**
 * Produce the `SLKey <fp>.<ts>.<b64sig>` Authorization header for a request, signing
 * over the canonical payload that binds the key fingerprint, current time, and the hash
 * of `body` (the exact serialized request body being sent).
 */
export async function buildSignedAuthHeader(
  info: KioskKeyInfo,
  body: string,
): Promise<string> {
  const timestamp = Math.floor(Date.now() / 1000);
  const bodyHashHex = await sha256Hex(new TextEncoder().encode(body));
  const payload = `${PAYLOAD_VERSION}:${info.fingerprint}:${timestamp}:${bodyHashHex}`;
  const sig = await crypto.subtle.sign(
    { name: "ECDSA", hash: "SHA-256" },
    info.keyPair.privateKey,
    new TextEncoder().encode(payload),
  );
  return `SLKey ${info.fingerprint}.${timestamp}.${bufToBase64(sig)}`;
}
