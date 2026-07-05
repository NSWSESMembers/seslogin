import { getCurrentClientVersion, GITHUB_REPO_URL } from "../lib/clientVersion";

function formatClientVersion(version: string): string {
  const normalized = version.trim();
  if (/^[0-9a-f]{40}$/i.test(normalized)) {
    return normalized.slice(0, 7);
  }
  return normalized;
}

export default function ClientVersionLabel({ noLink }: { noLink?: boolean }) {
  const currentVersion = getCurrentClientVersion();
  const normalized = currentVersion.trim();
  const displayVersion = formatClientVersion(currentVersion);

  if (!noLink && /^[0-9a-f]{40}$/i.test(normalized)) {
    return (
      <a
        href={`${GITHUB_REPO_URL}/commit/${normalized}`}
        target="_blank"
        rel="noopener noreferrer"
      >
        {displayVersion}
      </a>
    );
  }

  return <span>{displayVersion}</span>;
}
