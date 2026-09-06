export const CLIENT_VERSION_HEADER = "X-Client-Version";
export const GITHUB_REPO_URL = "https://github.com/NSWSESMembers/seslogin";

export function getCurrentClientVersion(): string {
  return import.meta.env.VITE_CLIENT_VERSION ?? "dev";
}

/**
 * Both the client build and the API server bake in a full 40-character git SHA (or
 * `"dev"` outside a git checkout). Shown in full, a SHA wraps mid-string in the kiosk
 * debug dialog and the admin footer alike, so every display site shortens it to the
 * same 7-character form GitHub uses for short revs.
 */
export function shortenGitRev(rev: string): string {
  const normalized = rev.trim();
  return /^[0-9a-f]{40}$/i.test(normalized)
    ? normalized.slice(0, 7)
    : normalized;
}
