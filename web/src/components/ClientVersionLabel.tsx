import {
  getCurrentClientVersion,
  GITHUB_REPO_URL,
  shortenGitRev,
} from "../lib/clientVersion";

export default function ClientVersionLabel({ noLink }: { noLink?: boolean }) {
  const currentVersion = getCurrentClientVersion();
  const normalized = currentVersion.trim();
  const displayVersion = shortenGitRev(currentVersion);

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
