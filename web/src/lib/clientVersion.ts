export const CLIENT_VERSION_HEADER = "X-Client-Version";
export const GITHUB_REPO_URL = "https://github.com/NSWSESMembers/seslogin";

export function getCurrentClientVersion(): string {
  return import.meta.env.VITE_CLIENT_VERSION ?? "dev";
}
