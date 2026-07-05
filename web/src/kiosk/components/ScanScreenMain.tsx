import type {
  TransactionState,
  Transaction as TransactionType,
  TransactionLoading as TransactionLoadingType,
  TransactionSignedIn as TransactionSignedInType,
  TransactionSignedOut as TransactionSignedOutType,
  TransactionError as TransactionErrorType,
} from "../ScanState";
import { formatTime, formatDayDateTime } from "../../lib/time";
import { useCallback, useEffect, useRef, useState } from "react";
import { scanView, scanViewPosition, type ScreenPosition } from "../../styles";
import { inputBase } from "../../components/ui/inputStyles";
import { Button } from "../../components/ui/Button";

// ensure this is less than the transaction timeout in ScanState
const FINALIZED_TRANSACTION_TIMEOUT_MS = 10_000;
const FINALIZED_TRANSACTION_FADE_MS = 1_000;
const SCAN_INPUT_TIMEOUT_MS = 10_000;

const transactionBase =
  "inline-block w-[800px] max-w-full rounded-md p-2.5 text-[1.2em] transition-opacity duration-1000";
const loadingSpinnerBase =
  "-mt-1.5 ml-2 inline-block size-[18px] rounded-full border-2 border-neutral-300 border-t-menu align-middle opacity-0";

function TransactionList(props: { transactionState: TransactionState }) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      setNow(Date.now());
    }, 250);

    return () => {
      window.clearInterval(intervalId);
    };
  }, []);

  return (
    <div className="mt-12.5">
      {props.transactionState.transactions
        .filter((t) => {
          if (
            t.status !== "SIGNED_IN" &&
            t.status !== "SIGNED_OUT" &&
            t.status !== "ERROR"
          ) {
            return true;
          }

          if (t.finalizedTime === undefined) {
            return true;
          }

          const elapsedMs = now - t.finalizedTime.getTime();
          return elapsedMs < FINALIZED_TRANSACTION_TIMEOUT_MS;
        })
        .map((t) => {
          let isFading = false;
          if (
            t.status === "SIGNED_IN" ||
            t.status === "SIGNED_OUT" ||
            t.status === "ERROR"
          ) {
            const elapsedMs =
              t.finalizedTime === undefined
                ? 0
                : now - t.finalizedTime.getTime();
            isFading =
              elapsedMs >=
              FINALIZED_TRANSACTION_TIMEOUT_MS - FINALIZED_TRANSACTION_FADE_MS;
          }

          return (
            <Transaction key={t.uuid} transaction={t} isFading={isFading} />
          );
        })}
    </div>
  );
}

function TransactionLoading(props: { transaction: TransactionLoadingType }) {
  return (
    <p>
      <span className={`${transactionBase} bg-yellow-300`}>
        Fetching information for {props.transaction.memberId}
      </span>
      <span
        className={`${loadingSpinnerBase} animate-spin opacity-100 motion-reduce:animate-none`}
      ></span>
    </p>
  );
}

function TransactionSignedIn(props: {
  transaction: TransactionSignedInType;
  isFading: boolean;
}) {
  const { transaction: txn, isFading } = props;
  return (
    <p>
      <span
        className={`${transactionBase} bg-green-300 ${isFading ? "opacity-0" : ""}`}
      >
        <span className="font-bold">
          {txn.person.firstName} {txn.person.lastName}
        </span>{" "}
        signed in at {formatTime(txn.startTime)}
      </span>
      <span className={loadingSpinnerBase}></span>
    </p>
  );
}

function TransactionSignedOut(props: {
  transaction: TransactionSignedOutType;
  isFading: boolean;
}) {
  const { transaction: txn, isFading } = props;
  // if startTime is not the current day, show the date
  const startTimeStr =
    txn.startTime.toDateString() === new Date().toDateString()
      ? formatTime(txn.startTime)
      : formatDayDateTime(txn.startTime);
  const endTimeStr =
    txn.endTime === undefined
      ? "?"
      : txn.endTime.toDateString() === new Date().toDateString()
        ? formatTime(txn.endTime)
        : formatDayDateTime(txn.endTime);
  return (
    <p>
      <span
        className={`${transactionBase} bg-green-300 ${isFading ? "opacity-0" : ""}`}
      >
        <span className="font-bold">
          {txn.person.firstName} {txn.person.lastName}
        </span>{" "}
        signed out: {startTimeStr} &ndash; {endTimeStr}
      </span>
      <span className={loadingSpinnerBase}></span>
    </p>
  );
}

function TransactionError(props: {
  transaction: TransactionErrorType;
  isFading: boolean;
}) {
  const { transaction: txn, isFading } = props;
  return (
    <p>
      <span
        className={`${transactionBase} bg-red-300 ${isFading ? "opacity-0" : ""}`}
      >
        <span className="font-bold">Error:</span> {txn.message}
      </span>
      <span className={loadingSpinnerBase}></span>
    </p>
  );
}

function Transaction(props: {
  transaction: TransactionType;
  isFading: boolean;
}) {
  const { transaction: txn, isFading } = props;

  if (txn.status === "LOADING") {
    return <TransactionLoading transaction={txn} />;
  } else if (txn.status === "SIGNED_IN") {
    return <TransactionSignedIn transaction={txn} isFading={isFading} />;
  } else if (txn.status === "SIGNED_OUT") {
    return <TransactionSignedOut transaction={txn} isFading={isFading} />;
  } else if (txn.status === "ERROR") {
    return <TransactionError transaction={txn} isFading={isFading} />;
  } else {
    throw new Error("Unknown transaction status");
  }
}

export default function ScanScreenMain(props: {
  screenPosition: ScreenPosition;
  submitDisabled: boolean;
  transactionState: TransactionState;
  onSubmit: (memberId: string) => Promise<void>;
  onFocusInputReady?: (focusInput: () => void) => void;
}) {
  const { onFocusInputReady, onSubmit, screenPosition, submitDisabled } = props;
  const inputRef = useRef<HTMLInputElement>(null);
  const refocusTimeoutIdRef = useRef<number | null>(null);
  const clearTimeoutIdRef = useRef<number | null>(null);

  const clearRefocusTimeout = useCallback(() => {
    if (refocusTimeoutIdRef.current !== null) {
      window.clearTimeout(refocusTimeoutIdRef.current);
      refocusTimeoutIdRef.current = null;
    }
  }, []);

  const clearInputTimeout = useCallback(() => {
    if (clearTimeoutIdRef.current !== null) {
      window.clearTimeout(clearTimeoutIdRef.current);
      clearTimeoutIdRef.current = null;
    }
  }, []);

  const focusInput = useCallback(() => {
    clearRefocusTimeout();
    inputRef.current?.focus();
  }, [clearRefocusTimeout]);

  const scheduleInputClearTimeout = useCallback(() => {
    clearInputTimeout();
    clearTimeoutIdRef.current = window.setTimeout(() => {
      if (inputRef.current !== null) {
        inputRef.current.value = "";
      }
      clearTimeoutIdRef.current = null;
    }, SCAN_INPUT_TIMEOUT_MS);
  }, [clearInputTimeout]);

  useEffect(() => {
    focusInput();

    return () => {
      clearRefocusTimeout();
      clearInputTimeout();
    };
  }, [clearInputTimeout, clearRefocusTimeout, focusInput]);

  useEffect(() => {
    onFocusInputReady?.(focusInput);
  }, [focusInput, onFocusInputReady]);

  async function handleSubmit(data: FormData) {
    const memberId = ((data.get("id") as string) ?? "").trim();
    if (memberId === "") {
      // Ignore empty submissions (e.g. Enter pressed on a blank/whitespace input)
      // so we never fire scanRegister2 with an empty registration number.
      focusInput();
      return;
    }
    await onSubmit(memberId);
  }

  return (
    <div className={`${scanView} ${scanViewPosition[screenPosition]}`}>
      <p className="mt-25 text-[2em]">Please enter or scan your SES ID</p>

      <form action={handleSubmit} autoComplete="off">
        <input
          ref={inputRef}
          type="text"
          name="id"
          maxLength={8}
          className={`${inputBase} mr-3.75 w-62.5 text-center align-middle font-mono text-[3em] transition-colors duration-500`}
          onBlur={() => {
            clearRefocusTimeout();
            refocusTimeoutIdRef.current = window.setTimeout(() => {
              if (
                inputRef.current !== null &&
                document.activeElement !== inputRef.current
              ) {
                inputRef.current.focus();
              }
              refocusTimeoutIdRef.current = null;
            }, SCAN_INPUT_TIMEOUT_MS);
          }}
          onFocus={() => {
            clearRefocusTimeout();
          }}
          onChange={() => {
            scheduleInputClearTimeout();
          }}
        />
        <Button
          variant="kiosk"
          size="bare"
          type="submit"
          className="h-16 w-17.5 text-[48px]"
          disabled={submitDisabled}
        >
          &gt;
        </Button>
      </form>

      <TransactionList transactionState={props.transactionState} />
    </div>
  );
}
