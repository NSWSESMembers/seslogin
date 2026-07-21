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
import { BadgeIcon } from "../../lib/badgeIcons";
import type { CSSProperties } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { scanView, scanViewPosition, type ScreenPosition } from "../../styles";
import { inputBase } from "../../components/ui/inputStyles";
import { Button } from "../../components/ui/Button";

// ensure this is less than the transaction timeout in ScanState
const FINALIZED_TRANSACTION_TIMEOUT_MS = 10_000;
const FINALIZED_TRANSACTION_FADE_MS = 1_000;
const SCAN_INPUT_TIMEOUT_MS = 10_000;
const CONFETTI_COLORS = [
  "#ff5f6d",
  "#ffd166",
  "#4ecdc4",
  "#5c7cfa",
  "#f8961e",
  "#ffffff",
] as const;

const transactionBase =
  "inline-block w-[800px] max-w-full rounded-md p-2.5 text-[1.2em] transition-opacity duration-1000";
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
        <BadgeCelebration badges={txn.awardedBadges} />
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
        <BadgeCelebration badges={txn.awardedBadges} />
      </span>
      <span className={loadingSpinnerBase}></span>
    </p>
  );
}

function BadgeCelebration(props: {
  badges:
    | ReadonlyArray<{
        id: string;
        name: string;
        description: string;
        tier: string;
      }>
    | null
    | undefined;
}) {
  type ConfettiPiece = {
    id: number;
    style: CSSProperties;
  };

  const [confettiPieces, setConfettiPieces] = useState<ConfettiPiece[]>([]);
  const [dismissedBadgeSignature, setDismissedBadgeSignature] = useState<
    string | null
  >(null);
  const audioFanfairRef = useRef<HTMLAudioElement | null>(null);
  const lastBadgeSignatureRef = useRef<string | null>(null);
  const badgeCount = props.badges?.length ?? 0;
  const badgeSignature =
    props.badges?.map((badge) => badge.id).join("|") ?? null;

  useEffect(() => {
    audioFanfairRef.current = new Audio("/audio/fanfair.mp3");
  }, []);

  useEffect(() => {
    if (!props.badges || props.badges.length === 0) {
      lastBadgeSignatureRef.current = null;
      return;
    }

    if (lastBadgeSignatureRef.current === badgeSignature) {
      return;
    }

    lastBadgeSignatureRef.current = badgeSignature;
    const audioFanfair = audioFanfairRef.current;
    if (audioFanfair) {
      audioFanfair.currentTime = 0;
      void audioFanfair.play().catch(() => undefined);
    }

    const burstCount = Math.max(24, props.badges.length * 16);
    setConfettiPieces(
      Array.from({ length: burstCount }, (_, id) => {
        const launchFromRight = id % 2 === 1;
        const horizontalOffset = 10 + Math.random() * 28;

        return {
          id,
          style: {
            "--confetti-origin-left": launchFromRight ? "auto" : "28px",
            "--confetti-origin-right": launchFromRight ? "28px" : "auto",
            "--confetti-x": `${launchFromRight ? -horizontalOffset : horizontalOffset}vw`,
            "--confetti-y": `${-22 - Math.random() * 42}vh`,
            "--confetti-rotation": `${-720 + Math.random() * 1440}deg`,
            "--confetti-delay": `${Math.random() * 160}ms`,
            "--confetti-duration": `${1200 + Math.random() * 900}ms`,
            "--confetti-size": `${6 + Math.random() * 8}px`,
            "--confetti-color": CONFETTI_COLORS[id % CONFETTI_COLORS.length],
          } as CSSProperties,
        };
      }),
    );

    const timeoutId = window.setTimeout(() => {
      setConfettiPieces([]);
    }, 2_600);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [badgeSignature, props.badges]);

  if (
    !props.badges ||
    props.badges.length === 0 ||
    badgeSignature === dismissedBadgeSignature
  ) {
    return null;
  }

  return (
    <span
      className="fixed inset-0 isolate z-[80] overflow-hidden"
      aria-live="polite"
    >
      <span
        className="fixed inset-0 z-0 cursor-pointer bg-[radial-gradient(circle_at_center,rgba(255,239,191,0.24)_0%,rgba(0,0,0,0.45)_65%,rgba(0,0,0,0.62)_100%)] backdrop-blur-[3px]"
        aria-hidden="true"
        onClick={() => {
          setDismissedBadgeSignature(badgeSignature);
        }}
      />
      <span
        className="pointer-events-none fixed inset-0 z-[1] overflow-visible"
        aria-hidden="true"
      >
        {confettiPieces.map((piece) => (
          <span
            key={piece.id}
            className="absolute right-[var(--confetti-origin-right,auto)] bottom-6.5 left-[var(--confetti-origin-left,28px)] h-[calc(var(--confetti-size)*0.55)] w-[var(--confetti-size)] origin-center animate-[badge-confetti-burst_var(--confetti-duration)_cubic-bezier(0.12,0.72,0.22,1)_var(--confetti-delay)_forwards] rounded-full bg-[var(--confetti-color)] opacity-0 shadow-[0_0_10px_rgba(255,255,255,0.22)] motion-reduce:animate-none motion-reduce:opacity-0"
            style={piece.style}
          />
        ))}
      </span>
      <span className="fixed top-1/2 left-1/2 z-[3] flex max-h-[calc(100vh-40px)] w-[min(880px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 flex-col items-stretch gap-3.5 overflow-auto rounded-3xl border-2 border-[rgba(255,228,170,0.45)] bg-linear-to-b from-[rgba(255,251,238,0.98)] to-[rgba(255,240,209,0.98)] px-6.5 pt-6 pb-5.5 shadow-[0_30px_80px_rgba(0,0,0,0.38),0_0_0_1px_rgba(255,255,255,0.5)_inset]">
        <span className="font-title text-[0.88rem] font-bold tracking-[0.14em] text-[#8c5300] uppercase">
          Badge awarded
        </span>
        <span className="font-title text-[clamp(2rem,4vw,3.6rem)] leading-[1.02] font-bold text-[#7f3f11] [text-shadow:0_1px_0_rgba(255,255,255,0.7)]">
          {badgeCount} {badgeCount === 1 ? "badge" : "badges"} unlocked
        </span>
        <span className="grid grid-cols-[repeat(auto-fit,minmax(260px,1fr))] gap-3">
          {props.badges.map((badge) => (
            <span
              key={badge.id}
              className="grid animate-[badge-pop_0.42s_ease-out] grid-cols-[54px_1fr] items-center gap-x-2.5 rounded-[18px] border-2 border-[#d57c2f] bg-linear-[120deg] from-[#fff3d8] via-[#ffe7c6] via-60% to-[#ffd59e] px-4 py-3.5 shadow-[0_10px_24px_rgba(212,124,47,0.22)] motion-reduce:animate-none"
            >
              <span className="inline-flex size-13 items-center justify-center">
                <BadgeIcon
                  badgeId={badge.id}
                  badgeName={badge.name}
                  tier={badge.tier.toLowerCase()}
                  className="block size-12"
                />
              </span>
              <span className="min-w-0">
                <span className="block text-[1.05rem] font-bold text-[#8a3f10]">
                  New badge: {badge.name}
                </span>
                <span className="block text-[0.98rem] leading-[1.35] text-[#6c2e09]">
                  {badge.description
                    ? `${badge.tier} tier - ${badge.description}`
                    : `${badge.tier} tier`}
                </span>
              </span>
            </span>
          ))}
        </span>
      </span>
    </span>
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
  validateMemberId: (memberId: string) => boolean;
  onFocusInputReady?: (focusInput: () => void) => void;
}) {
  const {
    onFocusInputReady,
    onSubmit,
    screenPosition,
    submitDisabled,
    validateMemberId,
  } = props;
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

  const clearInput = useCallback(() => {
    if (inputRef.current !== null) {
      inputRef.current.value = "";
    }
  }, []);

  const scheduleInputClearTimeout = useCallback(() => {
    clearInputTimeout();
    clearTimeoutIdRef.current = window.setTimeout(() => {
      clearInput();
      clearTimeoutIdRef.current = null;
    }, SCAN_INPUT_TIMEOUT_MS);
  }, [clearInputTimeout, clearInput]);

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

    clearInput();

    const isValidMemberId = validateMemberId(memberId);

    if (!isValidMemberId) {
      focusInput();
      return;
    }

    await onSubmit(memberId);
  }

  return (
    <div className={`${scanView} ${scanViewPosition[screenPosition]}`}>
      <p className="mt-25 text-[2em]">Please enter or scan your SES ID</p>

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
          maxLength={8}
          className={`${inputBase} mr-3.75 w-80 py-3 text-center align-middle font-mono text-[3em] leading-snug transition-colors duration-500`}
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
      </form>

      <TransactionList transactionState={props.transactionState} />
    </div>
  );
}
