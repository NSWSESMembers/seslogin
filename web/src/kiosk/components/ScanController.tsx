import { graphql, useMutation } from "react-relay";
import type { MemberIdWithUuid, TransactionSignedOut } from "../ScanState";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import { reducer } from "../ScanState";
import ScanScreenCategories from "./ScanScreenCategories";
import ScanScreenMain from "./ScanScreenMain";
import ScanScreenAdjust from "./ScanScreenAdjust";
import ScanScreenForgotSignOut from "./ScanScreenForgotSignOut";
import ScanGuestDialog from "./ScanGuestDialog";
import ScanStatusDialog from "./ScanStatusDialog";
import ScanScreenQuickPick from "./ScanScreenQuickPick";
import {
  blockClientUpdates,
  clearBlockClientUpdates,
} from "../../lib/clientUpdateLeases";
import type {
  ScanControllerRegister2Mutation,
  ScanControllerRegister2Mutation$data,
} from "./__generated__/ScanControllerRegister2Mutation.graphql";
import type {
  ScanControllerSignOutMutation,
  ScanControllerSignOutMutation$data,
} from "./__generated__/ScanControllerSignOutMutation.graphql";
import type {
  ScanControllerStartSignOutMutation,
  ScanControllerStartSignOutMutation$data,
} from "./__generated__/ScanControllerStartSignOutMutation.graphql";
import { useKioskSession } from "./useKioskSession";
import type { ScreenPosition } from "../../styles";
import { isValidMemberIdText } from "../../lib/memberId";
import {
  getErrorCode,
  getServerErrorMessage,
  isMutationFieldError,
} from "../../lib/relayErrors";
import { useSuspendScanFocus } from "../lib/scanFocusLeases";
import { useLivePeriods } from "./useLivePeriods";
import {
  interfaceModeFromConfig,
  resolveInterfaceMode,
} from "../lib/interfaceMode";

const PURGE_EXPIRED_TRANSACTIONS_INTERVAL_MS = 1_000;
const SCAN_TRANSACTION_LOG_LEASE_ID = "scan:transaction-log";
const SCAN_SCREEN_FOCUS_LEASE_ID = "scan:screen";

export default function ScanController(props: {
  onCancelSignOutChange?: (fn: (() => void) | null) => void;
  onSigningOutNameChange?: (name: string | null) => void;
}) {
  const session = useKioskSession();
  const { applyOwnResult, retry: refreshLivePeriods } = useLivePeriods();
  const guestsEnabled = !!session?.config?.guests;
  const quickPickCategories = !!session?.config?.quickPickCategories;
  const interfaceMode = interfaceModeFromConfig(session?.config?.interfaceMode);
  // Resolved once per config change rather than every render: the device
  // doesn't change under a running kiosk, and an auto guess that flipped
  // mid-entry would swap the input's behaviour under the member's finger.
  const touchInput = useMemo(
    () => resolveInterfaceMode(interfaceMode) === "touch",
    [interfaceMode],
  );
  const signedInStatus = !!session?.config?.signedInStatus;
  const signedInStatusInline = !!session?.config?.signedInStatusInline;
  // Rides on the always-visible list (see SessionForm), so it means nothing
  // without it.
  const signedInStatusSignOut =
    signedInStatusInline && !!session?.config?.signedInStatusSignOut;
  // A button that opens the very list already sitting on screen would be
  // pointless, so inline mode takes over from the button rather than the two
  // stacking (see SessionForm, which presents them as one choice).
  const signedInStatusButton = signedInStatus && !signedInStatusInline;

  const [transactionState, dispatchTransaction] = useReducer(reducer, {
    transactions: [],
  });
  const focusMainInputRef = useRef<(() => void) | null>(null);
  const listSignOutBusyRef = useRef(false);
  const [listSignOutBusy, setListSignOutBusy] = useState(false);
  const [guestDialogOpen, setGuestDialogOpen] = useState(false);
  const [statusDialogOpen, setStatusDialogOpen] = useState(false);

  // Opening the list must hand focus straight back to the member ID input: the
  // press that opened it left focus on the button, and the list holds no scan
  // focus lease precisely so a scan still lands while it is up. Without this,
  // the input only takes focus back on its own blur timer, and a scan in the
  // couple of seconds after the press would be typed into the button.
  const openStatusDialog = useCallback(() => {
    setStatusDialogOpen(true);
    focusMainInputRef.current?.();
  }, []);

  // Stable so ScanStatusDialog's auto-close timer isn't restarted by every
  // re-render of this controller — of which there are plenty, the transaction
  // purge timer alone firing once a second.
  const closeStatusDialog = useCallback(() => {
    setStatusDialogOpen(false);
    focusMainInputRef.current?.();
  }, []);

  // start periodically clearing old transactions
  useEffect(() => {
    const intervalId = window.setInterval(() => {
      dispatchTransaction({
        type: "PURGE_EXPIRED_TRANSACTIONS",
        now: new Date(),
      });
    }, PURGE_EXPIRED_TRANSACTIONS_INTERVAL_MS);

    return () => {
      window.clearInterval(intervalId);
    };
  }, []);

  useEffect(() => {
    const transactionCount = transactionState.transactions.length;
    if (transactionCount > 0) {
      blockClientUpdates(
        SCAN_TRANSACTION_LOG_LEASE_ID,
        `Scan transaction log has ${transactionCount} pending item(s)`,
      );
      return;
    }
    clearBlockClientUpdates(SCAN_TRANSACTION_LOG_LEASE_ID);
  }, [transactionState.transactions.length]);

  useEffect(() => {
    return () => {
      // Prevent stale scan state from blocking reloads after route changes.
      clearBlockClientUpdates(SCAN_TRANSACTION_LOG_LEASE_ID);
    };
  }, []);

  const audioSuccess = useMemo(() => new Audio("/audio/success.mp3"), []);
  const audioError = useMemo(() => new Audio("/audio/error.mp3"), []);

  const [commitRegister2Mutation] =
    useMutation<ScanControllerRegister2Mutation>(graphql`
      mutation ScanControllerRegister2Mutation(
        $memberNumber: String!
        $quickPick: Boolean!
      ) {
        scanRegister2(memberNumber: $memberNumber, quickPick: $quickPick) {
          state
          period {
            id
            version
            startTime
            endTime
            person {
              id
              firstName
              lastName
            }
          }
          quickPick {
            locationCategories {
              category {
                id
              }
              recentPeople {
                id
                firstName
              }
            }
            personCategories {
              category {
                id
              }
            }
          }
        }
      }
    `);
  // Start signing out one specific row of the signed-in list. Same result as a
  // scan of a signed-in member (state SIGN_OUT_PENDING), but it names the period
  // instead of a member number, so it can't sign anyone in and acts on the row
  // that was pressed. Nothing is written.
  const [commitStartSignOutMutation] =
    useMutation<ScanControllerStartSignOutMutation>(graphql`
      mutation ScanControllerStartSignOutMutation(
        $id: ID!
        $quickPick: Boolean!
      ) {
        scanStartSignOut(id: $id, quickPick: $quickPick) {
          state
          period {
            id
            version
            startTime
            endTime
            person {
              id
              firstName
              lastName
            }
          }
          quickPick {
            locationCategories {
              category {
                id
              }
              recentPeople {
                id
                firstName
              }
            }
            personCategories {
              category {
                id
              }
            }
          }
        }
      }
    `);
  const [commitSignOutMutation, signOutIsInFlight] =
    useMutation<ScanControllerSignOutMutation>(graphql`
      mutation ScanControllerSignOutMutation(
        $id: ID!
        $startTime: Int!
        $endTime: Int!
        $categoryId: ID!
      ) {
        scanSignOut(
          id: $id
          startTime: $startTime
          endTime: $endTime
          categoryId: $categoryId
        ) {
          id
          version
          person {
            id
            firstName
            lastName
          }
          startTime
          endTime
          category {
            id
            name
          }
        }
      }
    `);

  // The sign-out screens take over from here, whether the sign-out was started
  // by a scan or from the signed-in list.
  function resolveSignOutPending(
    uuid: string,
    result:
      | ScanControllerRegister2Mutation$data["scanRegister2"]
      | ScanControllerStartSignOutMutation$data["scanStartSignOut"],
  ) {
    const startTime = new Date(result.period!.startTime! * 1000);
    const quickPick = result.quickPick;
    dispatchTransaction({
      type: "PERSON_RESOLVED",
      uuid,
      periodId: result.period!.id,
      person: result.period!.person!,
      status: "SIGNED_OUT",
      startTime,
      // Null when this kiosk didn't ask for it, or when the server couldn't
      // build it — either way the sign-out screen shows the full category tree.
      quickPick: quickPick
        ? {
            location: quickPick.locationCategories.map((entry) => ({
              categoryId: entry.category.id,
              peopleNames: entry.recentPeople.map((p) => p.firstName),
            })),
            person: quickPick.personCategories.map((entry) => ({
              categoryId: entry.category.id,
            })),
          }
        : undefined,
    });
  }

  async function completeSubmit(ids: MemberIdWithUuid) {
    const { memberId, uuid } = ids;
    focusMainInputRef.current?.();

    let res: ScanControllerRegister2Mutation$data;
    try {
      res = await new Promise((resolve, reject) => {
        commitRegister2Mutation({
          // Only worth the server's extra reads if this kiosk shows the screen.
          variables: { memberNumber: memberId, quickPick: quickPickCategories },
          onCompleted: resolve,
          onError: reject,
        });
      });
    } catch (err) {
      console.error("Error during register2 mutation:", err);
      audioError.play();
      if (isMutationFieldError(err)) {
        // The scan was recorded server-side — only reading back its result (e.g.
        // the member's name) failed. Rescanning would create a second
        // transaction, so point the operator at admin instead of implying
        // nothing happened.
        dispatchTransaction({
          type: "ERROR",
          uuid,
          message: `recorded a scan for member ID ${memberId}, but couldn't display the result — check the activity list in admin`,
        });
        return;
      }
      // An unknown member ID comes back as a NOT_FOUND state, not an error, so a
      // rejection here is something the operator needs the detail of — e.g. two
      // people sharing a registration number.
      const serverMessage = getServerErrorMessage(err);
      dispatchTransaction({
        type: "ERROR",
        uuid,
        message: serverMessage
          ? `could not look up member ID ${memberId}: ${serverMessage}`
          : "network issue while looking up member ID: " + memberId,
      });
      return;
    }

    const state = res.scanRegister2.state;

    if (state == "NOT_FOUND") {
      audioError.play();
      dispatchTransaction({
        type: "ERROR",
        uuid,
        message: "Unknown member ID: " + memberId,
      });
      return;
    } else if (state == "SIGNED_IN") {
      audioSuccess.play();
      const period = res.scanRegister2.period!;
      const startTime = new Date(period.startTime! * 1000);
      const person = period.person!;
      applyOwnResult({
        kind: "opened",
        periodId: period.id,
        version: period.version,
        name: `${person.firstName} ${person.lastName}`,
        guest: false,
        startTime: period.startTime!,
      });
      dispatchTransaction({
        type: "PERSON_RESOLVED",
        uuid,
        periodId: period.id,
        person,
        status: "SIGNED_IN",
        startTime,
      });
      return;
    } else if (state == "SIGN_OUT_PENDING") {
      audioSuccess.play();
      resolveSignOutPending(uuid, res.scanRegister2);
      return;
    }

    console.log("Response:", res);
    throw new Error("Unknown scan state");
  }

  function handleValidateMemberId(memberId: string): boolean {
    if (!isValidMemberIdText(memberId)) {
      audioError.play();
      dispatchTransaction({
        type: "ABORT",
        message: "Member ID must be at least 8 digits long",
        uuid: undefined,
      });
      return false;
    }

    return true;
  }

  async function handleMemberIdEntered(memberId: string) {
    const uuid = crypto.randomUUID();

    // The signed-in list holds no scan focus lease, so a scan can land while it
    // is up. Get it out of the way before the result it would cover appears.
    setStatusDialogOpen(false);

    dispatchTransaction({ type: "LOAD_PERSON", uuid, memberId });

    // purposefully not awaited - we want the form submission to be considered complete
    // so we can re-render
    completeSubmit({ memberId, uuid });
  }

  // Sign someone out from the always-visible list. The list knows the period, so
  // ask the server to start the sign-out for exactly that row: the usual
  // forgot-to-sign-out / category / time steps follow, so a person still
  // confirms every sign-out.
  async function handleSignOutFromList(periodId: string, name: string) {
    // One at a time. The ref makes this hold for two presses in the same tick,
    // before a re-render could disable the buttons.
    if (
      listSignOutBusyRef.current ||
      transactionState.transactions[0]?.status === "LOADING"
    ) {
      return;
    }
    listSignOutBusyRef.current = true;
    setListSignOutBusy(true);
    // The press that triggered this left focus on the list's button; a scan
    // landing in the next moment would be typed into it and lost.
    focusMainInputRef.current?.();

    const uuid = crypto.randomUUID();
    dispatchTransaction({ type: "LOAD_PERSON", uuid, name });

    let res: ScanControllerStartSignOutMutation$data;
    try {
      res = await new Promise((resolve, reject) => {
        commitStartSignOutMutation({
          // Only worth the server's extra reads if this kiosk shows the screen.
          variables: { id: periodId, quickPick: quickPickCategories },
          onCompleted: resolve,
          onError: reject,
        });
      });
    } catch (err) {
      console.error("Error starting sign-out from the list:", err);
      audioError.play();
      // The server refuses a stale row with a code and a message written for the
      // kiosk ("That sign-in no longer exists", "Already signed out"). Nothing
      // was written, so there is nothing to clean up.
      const serverMessage = getServerErrorMessage(err);
      dispatchTransaction({
        type: "ERROR",
        uuid,
        message: serverMessage
          ? `could not sign out ${name}: ${serverMessage}`
          : "network issue while starting the sign-out for " + name,
      });
      // The row we pressed is out of date (already signed out, or gone), so the
      // list is too: fetch it again rather than leave a dead row to be pressed.
      const code = getErrorCode(err);
      if (code === "CONFLICT" || code === "NOT_FOUND") {
        refreshLivePeriods();
      }
      return;
    } finally {
      listSignOutBusyRef.current = false;
      setListSignOutBusy(false);
    }

    audioSuccess.play();
    resolveSignOutPending(uuid, res.scanStartSignOut);
  }

  // most recent transaction
  const newTransaction = transactionState.transactions[0];
  const memberIdEnabled = newTransaction?.status != "LOADING";
  const signedOutTransaction: TransactionSignedOut | null =
    newTransaction?.status === "SIGNED_OUT" ? newTransaction : null;
  // The register mutation returns these with the sign-out, so we know up front
  // whether there is anything to show — skip the screen entirely rather than
  // sliding an empty one in. (Suggestions the static category tree doesn't
  // recognise are dropped later, inside the screen, which skips itself if that
  // leaves nothing.)
  const quickPickSuggestions = signedOutTransaction?.quickPick ?? null;
  const hasQuickPickSuggestions =
    !!quickPickSuggestions &&
    (quickPickSuggestions.location.length > 0 ||
      quickPickSuggestions.person.length > 0);
  // Shown before quick pick / categories when someone scans to sign out after
  // being signed in for an implausibly long stretch — they probably left without
  // signing out. They pick a sensible end time and the flow carries on to the
  // category screen as normal.
  const needsForgotSignOut =
    typeof newTransaction !== "undefined" &&
    newTransaction.status == "SIGNED_OUT" &&
    typeof newTransaction.categoryId === "undefined" &&
    newTransaction.longSession &&
    !newTransaction.forgotSignOutPrompted;
  const needsQuickPick =
    quickPickCategories &&
    hasQuickPickSuggestions &&
    typeof newTransaction !== "undefined" &&
    newTransaction.status == "SIGNED_OUT" &&
    typeof newTransaction.categoryId === "undefined" &&
    !newTransaction.quickPickSkipped &&
    !needsForgotSignOut;
  const needsCategory =
    typeof newTransaction !== "undefined" &&
    newTransaction.status == "SIGNED_OUT" &&
    typeof newTransaction.categoryId === "undefined" &&
    !needsQuickPick &&
    !needsForgotSignOut;
  const needsAdjust =
    typeof newTransaction !== "undefined" &&
    newTransaction.status == "SIGNED_OUT" &&
    !newTransaction.adjusted &&
    !needsForgotSignOut;

  // we use this as a key to ensure ScanCategories/ScanQuickPick clear state for each transaction
  const transactionUuid =
    needsForgotSignOut || needsQuickPick || needsCategory || needsAdjust
      ? newTransaction.uuid
      : null;

  // Refs so onSubmitAdjust always reads latest values at call time regardless of memoization.
  // Synced in useLayoutEffect (not during render) to be safe under concurrent rendering.
  const signedOutTransactionRef = useRef(signedOutTransaction);
  const transactionUuidRef = useRef(transactionUuid);
  useLayoutEffect(() => {
    signedOutTransactionRef.current = signedOutTransaction;
    transactionUuidRef.current = transactionUuid;
  });

  const mainPos: ScreenPosition =
    needsForgotSignOut || needsQuickPick || needsCategory || needsAdjust
      ? "offLeft"
      : "center";
  const forgotSignOutPos: ScreenPosition = needsForgotSignOut
    ? "center"
    : "offRight";
  const quickPickPos: ScreenPosition = needsQuickPick ? "center" : "offRight";
  const categoriesPos: ScreenPosition = needsCategory ? "center" : "offRight";
  const adjustPos: ScreenPosition =
    !needsForgotSignOut && !needsQuickPick && !needsCategory && needsAdjust
      ? "center"
      : "offRight";

  // The main screen (and its still-mounted member ID input) is slid off to the
  // side while quick pick / categories / adjust are up, so its refocus timer
  // must not pull focus out of whatever is on screen.
  useSuspendScanFocus(SCAN_SCREEN_FOCUS_LEASE_ID, mainPos !== "center");

  const onCancelSignOut = useCallback(() => {
    if (!transactionUuid) return;
    dispatchTransaction({ type: "CANCEL_TRANSACTION", uuid: transactionUuid });
    focusMainInputRef.current?.();
  }, [transactionUuid]);

  const canCancelSignOut =
    needsForgotSignOut || needsQuickPick || needsCategory || needsAdjust;
  const { onCancelSignOutChange } = props;
  useEffect(() => {
    onCancelSignOutChange?.(canCancelSignOut ? onCancelSignOut : null);
  }, [canCancelSignOut, onCancelSignOut, onCancelSignOutChange]);

  const signingOutName =
    (needsForgotSignOut || needsQuickPick || needsCategory || needsAdjust) &&
    signedOutTransaction
      ? `${signedOutTransaction.person.firstName} ${signedOutTransaction.person.lastName}`
      : null;
  const { onSigningOutNameChange } = props;
  useEffect(() => {
    onSigningOutNameChange?.(signingOutName);
  }, [signingOutName, onSigningOutNameChange]);

  function onSelectCategory(uuid: string, categoryId: string) {
    dispatchTransaction({ type: "SET_CATEGORY", uuid, categoryId });
  }

  function onResolveForgotSignOut(endTime?: Date) {
    if (!transactionUuid) return;
    dispatchTransaction({
      type: "RESOLVE_FORGOT_SIGN_OUT",
      uuid: transactionUuid,
      endTime,
    });
  }

  // Memoized: passed down into ScanScreenQuickPick's suspended query tree, where
  // an unstable reference would retrigger its "no recent categories" auto-skip effect.
  const onSkipQuickPick = useCallback(() => {
    if (!transactionUuidRef.current) return;
    dispatchTransaction({
      type: "SKIP_QUICK_PICK",
      uuid: transactionUuidRef.current,
    });
  }, []);

  function onEditCategory() {
    dispatchTransaction({
      type: "CLEAR_CATEGORY",
      uuid: transactionUuid!,
    });
  }

  function onSubmitAdjust(startTime: Date, endTime: Date) {
    const tx = signedOutTransactionRef.current;
    const uuid = transactionUuidRef.current;
    if (!tx || !uuid) return;
    const variables = {
      id: tx.periodId,
      startTime: Math.floor(startTime.getTime() / 1000),
      endTime: Math.floor(endTime.getTime() / 1000),
      categoryId: tx.categoryId!,
    };
    const onCompleted = (data: ScanControllerSignOutMutation$data) => {
      console.log("Adjust mutation completed");
      const period = data.scanSignOut;
      const person = period.person!;
      applyOwnResult({
        kind: "closed",
        periodId: period.id,
        version: period.version,
        name: `${person.firstName} ${person.lastName}`,
        guest: false,
        startTime: period.startTime,
        endTime: period.endTime ?? null,
        deleted: false,
      });
      dispatchTransaction({
        type: "ADJUST_PERIOD",
        uuid,
        startTime,
        endTime,
      });
      focusMainInputRef.current?.();
    };
    const onError = (err: Error) => {
      console.error("Error during adjust mutation:", err);
      audioError.play();
      const name = tx.person.firstName + " " + tx.person.lastName;
      if (isMutationFieldError(err)) {
        // The sign-out was recorded server-side — only reading back its result
        // failed. Repeating the action would create a second record, so point
        // the operator at admin instead of implying nothing happened.
        dispatchTransaction({
          type: "ERROR",
          uuid,
          message: `recorded the sign-out for ${name}, but couldn't display the result — check the activity list in admin`,
        });
        return;
      }
      // A rejected mutation (e.g. start time after end time) carries the server's
      // message; only a genuine transport failure gets the generic network wording.
      const serverMessage = getServerErrorMessage(err);
      dispatchTransaction({
        type: "ERROR",
        uuid,
        message: serverMessage
          ? `could not sign out ${name}: ${serverMessage}`
          : "network issue while adjusting record for " + name,
      });
    };
    commitSignOutMutation({ variables, onCompleted, onError });
  }

  return (
    <>
      <ScanScreenMain
        screenPosition={mainPos}
        transactionState={transactionState}
        submitDisabled={!memberIdEnabled}
        validateMemberId={handleValidateMemberId}
        onSubmit={handleMemberIdEntered}
        onFocusInputReady={(focusInput) => {
          focusMainInputRef.current = focusInput;
        }}
        guestsEnabled={guestsEnabled}
        onOpenGuestDialog={() => setGuestDialogOpen(true)}
        touchInput={touchInput}
        statusEnabled={signedInStatusButton}
        onOpenStatusDialog={openStatusDialog}
        signedInInline={signedInStatusInline}
        onSignOutFromList={
          signedInStatusSignOut ? handleSignOutFromList : undefined
        }
        // transactionUuid is set only while a sign-out screen is up (a finished
        // sign-out stays in the log for a minute but no longer blocks the list).
        signOutFromListDisabled={
          listSignOutBusy || !memberIdEnabled || transactionUuid !== null
        }
      />
      <ScanScreenForgotSignOut
        screenPosition={forgotSignOutPos}
        transaction={signedOutTransaction}
        uuid={needsForgotSignOut ? transactionUuid : null}
        onResolve={onResolveForgotSignOut}
      />
      <ScanScreenQuickPick
        screenPosition={quickPickPos}
        onSelectCategory={onSelectCategory}
        onSkip={onSkipQuickPick}
        uuid={needsQuickPick ? transactionUuid : null}
        suggestions={quickPickSuggestions}
      />
      <ScanScreenCategories
        screenPosition={categoriesPos}
        onSelectCategory={onSelectCategory}
        uuid={transactionUuid}
      />
      <ScanScreenAdjust
        screenPosition={adjustPos}
        onSubmit={onSubmitAdjust}
        onError={() => audioError.play()}
        uuid={transactionUuid}
        transaction={signedOutTransaction}
        onEditCategory={onEditCategory}
        isSubmitting={signOutIsInFlight}
      />
      {statusDialogOpen && <ScanStatusDialog onClose={closeStatusDialog} />}
      {guestDialogOpen && (
        <ScanGuestDialog
          onClose={() => {
            setGuestDialogOpen(false);
            focusMainInputRef.current?.();
          }}
        />
      )}
    </>
  );
}
