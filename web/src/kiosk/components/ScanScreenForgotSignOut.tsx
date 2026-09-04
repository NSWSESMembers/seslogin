import { useState } from "react";
import type { TransactionSignedOut } from "../ScanState";
import { formatFullDateTime, formatSeconds } from "../../lib/time";
import { scanViewProps, type ScreenPosition } from "../../styles";
import { Button } from "../../components/ui/Button";

const ONE_HOUR_MS = 60 * 60 * 1000;

function Inner(props: {
  transaction: TransactionSignedOut;
  onResolve: (endTime?: Date) => void;
}) {
  const { transaction, onResolve } = props;
  // Captured once on mount — Inner is remounted per transaction (keyed on uuid),
  // and the wording doesn't need to tick.
  const [now] = useState(() => new Date());

  const signedInFor = formatSeconds(
    Math.round((now.getTime() - transaction.startTime.getTime()) / 1000),
  );
  const oneHourAfter = new Date(transaction.startTime.getTime() + ONE_HOUR_MS);

  return (
    <>
      <h1 className="m-0 mb-4 text-[3em]">Did you forget to sign out?</h1>
      <p className="m-0 mb-2 text-[1.6em]">You&apos;ve been signed in since</p>
      <p className="m-0 mb-6 text-[2em] font-bold">
        {formatFullDateTime(transaction.startTime)}
      </p>
      <p className="m-0 mb-8 max-w-3xl text-[1.4em]">
        That&apos;s about {signedInFor} ago. If you pick "Yeah", we&apos;ll
        back-date your sign-out to one hour after you started this session.
        You&apos;ll still need to pick a category for the entry we're closing
        out and then you'll be offered a chance to adjust your sign-in and
        sign-out times as normal. After all of that you can sign yourself back
        in if you wish to start a new entry.
      </p>
      <div className="flex items-center justify-center gap-4">
        <Button
          variant="kiosk"
          size="bare"
          className="px-8 py-3 text-[1.6em]"
          onClick={() => onResolve(oneHourAfter)}
        >
          Yeah, I forgot
        </Button>
        <Button
          variant="kiosk"
          size="bare"
          className="px-8 py-3 text-[1.6em]"
          onClick={() => onResolve()}
        >
          Nope, this is a long entry
        </Button>
      </div>
    </>
  );
}

// we expose this wrapper just so we can reset inner state on UUID change without
// causing the container <div> to remount and lose CSS transition state
export default function ScanScreenForgotSignOut(props: {
  transaction: TransactionSignedOut | null;
  uuid: string | null;
  screenPosition: ScreenPosition;
  onResolve: (endTime?: Date) => void;
}) {
  return (
    <div
      {...scanViewProps(
        props.screenPosition,
        "inset-y-0 flex flex-col items-center justify-center text-center",
      )}
    >
      {props.uuid && props.transaction && (
        <Inner
          key={props.uuid}
          transaction={props.transaction}
          onResolve={props.onResolve}
        />
      )}
    </div>
  );
}
