import type {
  TransactionState,
  Transaction as TransactionType,
  TransactionLoading as TransactionLoadingType,
  TransactionSignedIn as TransactionSignedInType,
  TransactionSignedOut as TransactionSignedOutType,
  TransactionError as TransactionErrorType,
  TransactionAborted as TransactionAbortedType,
} from "../ScanState";
import { formatTime, formatDayDateTime } from "../../lib/time";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { scanViewProps, type ScreenPosition } from "../../styles";
import { inputBase } from "../../components/ui/inputStyles";
import { Button } from "../../components/ui/Button";
import {
  isScanFocusSuspended,
  onScanFocusSuspendedChange,
} from "../lib/scanFocusLeases";
import ScanNumberPadDialog from "./ScanNumberPadDialog";
// The input caps typing at the member ID length; the number pad has to enforce
// it itself, because setting `value` from script bypasses `maxLength`.
import { MEMBER_ID_LENGTH } from "../../lib/memberId";
import ScanSignedInPanel from "./ScanSignedInPanel";

// ensure this is less than the transaction timeout in ScanState
const FINALIZED_TRANSACTION_TIMEOUT_MS = 10_000;
const FINALIZED_TRANSACTION_FADE_MS = 1_000;
// How long a half-typed member ID is left in the input before it is discarded.
const SCAN_INPUT_CLEAR_TIMEOUT_MS = 10_000;
// How long after the input loses focus before the scan screen takes it back, so
// a scan is never typed into nothing. Suspended while an overlay holds a scan
// focus lease — see ../lib/scanFocusLeases.
const SCAN_INPUT_REFOCUS_TIMEOUT_MS = 2_000;

const transactionBase =
  "inline-block w-[800px] max-w-full rounded-md p-2.5 text-xl transition-opacity duration-1000";
const loadingSpinnerBase =
  "-mt-1.5 ml-2 inline-block size-[18px] rounded-full border-2 border-line border-t-menu align-middle opacity-0";

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
        .map((t, idx) => {
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
            <Transaction
              key={t.uuid || idx}
              transaction={t}
              isFading={isFading}
            />
          );
        })}
    </div>
  );
}

function TransactionLoading(props: { transaction: TransactionLoadingType }) {
  return (
    <p>
      <span
        className={`${transactionBase} bg-yellow-300 dark:bg-yellow-700 dark:text-white`}
      >
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
        className={`${transactionBase} bg-green-300 dark:bg-green-700 dark:text-white ${isFading ? "opacity-0" : ""}`}
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
        className={`${transactionBase} bg-green-300 dark:bg-green-700 dark:text-white ${isFading ? "opacity-0" : ""}`}
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
  transaction: TransactionErrorType | TransactionAbortedType;
  isFading: boolean;
}) {
  const { transaction: txn, isFading } = props;
  return (
    <p>
      <span
        className={`${transactionBase} bg-red-300 dark:bg-red-700 dark:text-white ${isFading ? "opacity-0" : ""}`}
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
  validateMemberId: (memberId: string) => boolean;
  onFocusInputReady?: (focusInput: () => void) => void;
  guestsEnabled?: boolean;
  onOpenGuestDialog?: () => void;
  numberPadEnabled?: boolean;
  statusEnabled?: boolean;
  onOpenStatusDialog?: () => void;
  signedInInline?: boolean;
}) {
  const {
    onFocusInputReady,
    onSubmit,
    screenPosition,
    submitDisabled,
    validateMemberId,
    guestsEnabled,
    onOpenGuestDialog,
    numberPadEnabled,
    statusEnabled,
    onOpenStatusDialog,
    signedInInline,
  } = props;
  const inputRef = useRef<HTMLInputElement>(null);
  const refocusTimeoutIdRef = useRef<number | null>(null);
  const clearTimeoutIdRef = useRef<number | null>(null);
  const [padOpen, setPadOpen] = useState(false);
  // A mirror of the input's text, kept only for what React has to render from
  // it: the pad's own digit display, and whether the button beside the input is
  // the submit arrow or the one that opens the pad. The input itself stays the
  // source of truth — every edit goes through it first.
  const [memberIdText, setMemberIdText] = useState("");
  const focusSuspended = useSyncExternalStore(
    onScanFocusSuspendedChange,
    isScanFocusSuspended,
  );

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

  // Read the lease state live rather than through the render snapshot: this runs
  // from timers and callbacks that outlive the render they were created in.
  const focusInput = useCallback(() => {
    clearRefocusTimeout();
    if (isScanFocusSuspended()) {
      return;
    }
    inputRef.current?.focus();
  }, [clearRefocusTimeout]);

  const syncMemberIdText = useCallback(() => {
    setMemberIdText(inputRef.current?.value ?? "");
  }, []);

  const clearInput = useCallback(() => {
    if (inputRef.current !== null) {
      inputRef.current.value = "";
    }
    syncMemberIdText();
  }, [syncMemberIdText]);

  const scheduleInputClearTimeout = useCallback(() => {
    clearInputTimeout();
    clearTimeoutIdRef.current = window.setTimeout(() => {
      clearInput();
      clearTimeoutIdRef.current = null;
    }, SCAN_INPUT_CLEAR_TIMEOUT_MS);
  }, [clearInputTimeout, clearInput]);

  // Focus on mount, and take focus back whenever the last overlay closes.
  useEffect(() => {
    if (focusSuspended) {
      clearRefocusTimeout();
      return;
    }
    focusInput();
  }, [clearRefocusTimeout, focusInput, focusSuspended]);

  useEffect(() => {
    return () => {
      clearRefocusTimeout();
      clearInputTimeout();
    };
  }, [clearInputTimeout, clearRefocusTimeout]);

  useEffect(() => {
    onFocusInputReady?.(focusInput);
  }, [focusInput, onFocusInputReady]);

  // Every way of submitting ends up here — the button beside the input, Enter in
  // the input (a barcode scanner's trailing newline included) and the pad's own
  // Enter key — so the pad closing on submit is a property of submitting rather
  // than of the key that was pressed.
  async function submitMemberId(rawMemberId: string) {
    const memberId = rawMemberId.trim();
    setPadOpen(false);
    if (memberId === "") {
      // Ignore empty submissions (e.g. Enter pressed on a blank/whitespace input)
      // so we never fire scanRegister2 with an empty registration number.
      focusInput();
      return;
    }

    clearInput();

    const isValidMemberId = validateMemberId(memberId);

    if (!isValidMemberId) {
      focusInput();
      return;
    }

    await onSubmit(memberId);
  }

  async function handleSubmit(data: FormData) {
    await submitMemberId((data.get("id") as string) ?? "");
  }

  // The number pad edits the input's value directly rather than holding the
  // typed ID in state: the input stays the single source of truth, so a pad
  // press, a barcode scan and a keyboard can be mixed on the same entry. Each
  // press also restarts the clear timeout, which only an `onChange` from real
  // typing would otherwise do — a half-tapped ID left on screen is discarded on
  // the same timer as a half-typed one.
  function handleNumberPadDigit(digit: string) {
    const input = inputRef.current;
    if (input === null || input.value.length >= MEMBER_ID_LENGTH) {
      return;
    }
    input.value = input.value + digit;
    syncMemberIdText();
    scheduleInputClearTimeout();
    focusInput();
  }

  function handleNumberPadDelete() {
    const input = inputRef.current;
    if (input === null) {
      return;
    }
    input.value = input.value.slice(0, -1);
    syncMemberIdText();
    scheduleInputClearTimeout();
    focusInput();
  }

  function handleNumberPadSubmit() {
    clearInputTimeout();
    submitMemberId(inputRef.current?.value ?? "");
  }

  const showPadButton = !!numberPadEnabled && memberIdText === "";

  const mainColumn = (
    <>
      <p className="mt-25 text-3xl">Please enter or scan your SES ID</p>

      <form
        autoComplete="off"
        onSubmit={(submitEvent) => {
          submitEvent.preventDefault();
          handleSubmit(new FormData(submitEvent.target));
        }}
      >
        <input
          ref={inputRef}
          type="text"
          name="id"
          maxLength={MEMBER_ID_LENGTH}
          className={`${inputBase} mr-3.75 w-80 py-3 text-center align-middle font-mono text-5xl/snug transition-colors duration-500`}
          onBlur={() => {
            clearRefocusTimeout();
            if (isScanFocusSuspended()) {
              return;
            }
            refocusTimeoutIdRef.current = window.setTimeout(() => {
              refocusTimeoutIdRef.current = null;
              // Re-checked here as well as above: the blur that started this
              // timer is what hands focus to an overlay, so the overlay's lease
              // is usually not registered yet when the timer is scheduled.
              if (isScanFocusSuspended()) {
                return;
              }
              if (
                inputRef.current !== null &&
                document.activeElement !== inputRef.current
              ) {
                inputRef.current.focus();
              }
            }, SCAN_INPUT_REFOCUS_TIMEOUT_MS);
          }}
          onFocus={() => {
            clearRefocusTimeout();
          }}
          onChange={() => {
            syncMemberIdText();
            scheduleInputClearTimeout();
          }}
        />
        {/* With nothing typed there is nothing to submit, so the button offers
            the pad instead — which is the only way in on a kiosk with no
            keyboard and no scanner. It turns back into Submit as soon as there
            is an ID, whether that came from the pad, a keyboard or a scan. */}
        {showPadButton ? (
          <Button
            variant="kiosk"
            size="bare"
            type="button"
            className="inline-flex h-16 w-17.5 items-center justify-center"
            aria-label="Number pad"
            onClick={() => {
              setPadOpen(true);
              focusInput();
            }}
          >
            <svg
              aria-hidden="true"
              viewBox="0 0 24 24"
              fill="currentColor"
              className="size-8"
            >
              <circle cx="6" cy="6" r="1.9" />
              <circle cx="12" cy="6" r="1.9" />
              <circle cx="18" cy="6" r="1.9" />
              <circle cx="6" cy="12" r="1.9" />
              <circle cx="12" cy="12" r="1.9" />
              <circle cx="18" cy="12" r="1.9" />
              <circle cx="6" cy="18" r="1.9" />
              <circle cx="12" cy="18" r="1.9" />
              <circle cx="18" cy="18" r="1.9" />
            </svg>
          </Button>
        ) : (
          <Button
            variant="kiosk"
            size="bare"
            type="submit"
            className="inline-flex h-16 w-17.5 items-center justify-center"
            disabled={submitDisabled}
            aria-label="Submit"
          >
            <svg
              aria-hidden="true"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={3}
              strokeLinecap="round"
              strokeLinejoin="round"
              className="size-8"
            >
              <path d="M9 5l7 7-7 7" />
            </svg>
          </Button>
        )}
      </form>

      {padOpen && (
        <ScanNumberPadDialog
          value={memberIdText}
          onDigit={handleNumberPadDigit}
          onDelete={handleNumberPadDelete}
          onSubmit={handleNumberPadSubmit}
          onClose={() => {
            setPadOpen(false);
            focusInput();
          }}
          submitDisabled={submitDisabled || memberIdText === ""}
        />
      )}

      {((guestsEnabled && onOpenGuestDialog) ||
        (statusEnabled && onOpenStatusDialog)) && (
        <div className="mt-6 flex flex-wrap justify-center gap-3">
          {guestsEnabled && onOpenGuestDialog && (
            <Button variant="kiosk" type="button" onClick={onOpenGuestDialog}>
              Guest sign in / out
            </Button>
          )}
          {statusEnabled && onOpenStatusDialog && (
            <Button variant="kiosk" type="button" onClick={onOpenStatusDialog}>
              Who's here
            </Button>
          )}
        </div>
      )}

      <TransactionList transactionState={props.transactionState} />
    </>
  );

  if (!signedInInline) {
    return <div {...scanViewProps(screenPosition)}>{mainColumn}</div>;
  }

  // The panel sits beside the input rather than the input's usual centred
  // column growing to fill the screen, so a wide kiosk doesn't stretch the
  // member ID field across it — the field's width is fixed either way.
  return (
    <div {...scanViewProps(screenPosition)}>
      <div className="mx-auto flex max-w-6xl items-start justify-center gap-10">
        <div className="min-w-0 flex-1">{mainColumn}</div>
        <div className="w-80 shrink-0 border-l border-line pt-25 pl-8">
          <ScanSignedInPanel />
        </div>
      </div>
    </div>
  );
}
