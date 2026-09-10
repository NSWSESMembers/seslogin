// A kiosk key fingerprint is the hex SHA-256 of the device's public key (see
// kiosk/lib/kioskKey.ts). Everywhere it's shown to a person it's truncated to
// this many characters — the kiosk prints the same prefix under its enrollment
// QR code, so an admin can compare the two by eye.
const SHORT_LEN = 16;

export function shortFingerprint(fingerprint: string) {
  return `${fingerprint.slice(0, SHORT_LEN)}…`;
}

// Full fingerprint in groups of four, so the whole 64-hex string stays readable
// when someone opens the popover to check every character.
export function groupFingerprint(fingerprint: string) {
  return fingerprint.replace(/(.{4})(?=.)/g, "$1 ");
}
