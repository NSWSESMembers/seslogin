import { useRef, useState } from "react";
import { getCurrentEnvironment } from "../../lib/clientInfo";
import { Popover } from "../../components/ui/Popover";

/**
 * The build channel a kiosk last reported, flagged when it isn't the one this admin page
 * was served from.
 *
 * This is the column that earns its place: the `test` and `preprod` front-ends talk to
 * the *production* database, so a kiosk running one of those builds is signing real
 * members in and out from a non-production deployment. Nothing else in the kiosk list
 * would show that — `lastContact` and `clientVersion` both look perfectly healthy.
 *
 * A kiosk that has never reported shows "—": it's on a client build predating this, not
 * necessarily in trouble.
 */
/**
 * The host part of a reported origin, or null if it isn't a URL. The value is whatever
 * the kiosk sent — the server bounds its length but never parses it — so this must not
 * be able to throw a malformed string into the middle of the kiosk list.
 */
function hostOf(origin: string | null | undefined): string | null {
  if (!origin) {
    return null;
  }
  try {
    return new URL(origin).host;
  } catch {
    return origin;
  }
}

// The environment-mismatch badge as a trigger: the explanation is on the `title`
// (hover) and in a popover on click, so it also works on touch — same pattern as
// CommentIndicator.
function EnvironmentMismatch({
  env,
  expected,
  origin,
}: {
  env: string;
  expected: string;
  origin: string | null | undefined;
}) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const explanation = `This kiosk is running the "${env}" build, but you are viewing the "${expected}" admin site.${
    origin ? ` It loaded from ${origin}.` : ""
  } Non-production builds still read and write the production database.`;

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        title={explanation}
        aria-label={open ? "Hide details" : "Show details"}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="cursor-help font-semibold text-amber-700 underline decoration-dotted underline-offset-2 dark:text-amber-400"
      >
        {env} ⚠
      </button>
      {open && (
        <Popover
          anchorRef={buttonRef}
          onDismiss={() => setOpen(false)}
          className="max-w-xs px-2 py-1.5 text-sm"
        >
          {explanation}
        </Popover>
      )}
    </>
  );
}

export default function SessionEnvironment({
  clientInfo,
}: {
  clientInfo:
    | { readonly env?: string | null; readonly origin?: string | null }
    | null
    | undefined;
}) {
  const env = clientInfo?.env;
  if (!env) {
    // Fall back to the origin — it's the same fact, less prettily, and a client can
    // report one without the other.
    const origin = clientInfo?.origin;
    return (
      <span className="text-ink-muted" title={origin ?? undefined}>
        {hostOf(origin) ?? "—"}
      </span>
    );
  }

  const expected = getCurrentEnvironment();
  if (env === expected) {
    return <span>{env}</span>;
  }

  return (
    <EnvironmentMismatch
      env={env}
      expected={expected}
      origin={clientInfo?.origin}
    />
  );
}
