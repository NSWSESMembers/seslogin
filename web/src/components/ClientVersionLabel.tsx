import { getCurrentClientVersion, GITHUB_REPO_URL } from "../lib/clientVersion";

function formatClientVersion(version: string): string {
  const normalized = version.trim();
  if (/^[0-9a-f]{40}$/i.test(normalized)) {
    return normalized.slice(0, 7);
  }
  return normalized;
}

export default function ClientVersionLabel() {
  const currentVersion = getCurrentClientVersion();
  const normalized = currentVersion.trim();
  const displayVersion = formatClientVersion(currentVersion);

  if (/^[0-9a-f]{40}$/i.test(normalized)) {
    return (
      <a
        className="client-version"
        href={`${GITHUB_REPO_URL}/commit/${normalized}`}
        target="_blank"
        rel="noopener noreferrer"
      >
        {displayVersion}
      </a>
    );
  }

  return <span className="client-version">{displayVersion}</span>;
}
