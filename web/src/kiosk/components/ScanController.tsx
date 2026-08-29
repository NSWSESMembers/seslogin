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
import ScanScreenQuickPick from "./ScanScreenQuickPick";
import {
  blockClientUpdates,
  clearBlockClientUpdates,
} from "../../lib/clientUpdateLeases";
import type {
  ScanControllerRegister2Mutation,
  ScanControllerRegister2Mutation$data,
} from "./__generated__/ScanControllerRegister2Mutation.graphql";
import type { ScanControllerSignOutMutation } from "./__generated__/ScanControllerSignOutMutation.graphql";
import { useKioskSession } from "./useKioskSession";
import type { ScreenPosition } from "../../styles";
import { isValidMemberIdText } from "../../lib/memberId";
import {
  getServerErrorMessage,
  isMutationFieldError,
} from "../../lib/relayErrors";
import { useSuspendScanFocus } from "../lib/scanFocusLeases";

const PURGE_EXPIRED_TRANSACTIONS_INTERVAL_MS = 1_000;
const SCAN_TRANSACTION_LOG_LEASE_ID = "scan:transaction-log";
const SCAN_SCREEN_FOCUS_LEASE_ID = "scan:screen";

export default function ScanController(props: {
  onCancelSignOutChange?: (fn: (() => void) | null) => void;
  onSigningOutNameChange?: (name: string | null) => void;
}) {
  const session = useKioskSession();
  const smallCategories = !!session?.config?.smallCategories;
  const easyTimeEntry = !!session?.config?.easyTimeEntry;
  // The reworked category list is now on for every kiosk; the `newCategories`
  // session config flag is ignored and is on its way out.
  const newCategories = true;
  const guestsEnabled = !!session?.config?.guests;
  const quickPickCategories = !!session?.config?.quickPickCategories;

  const [transactionState, dispatchTransaction] = useReducer(reducer, {
    transactions: [],
  });
  const focusMainInputRef = useRef<(() => void) | null>(null);
  const [guestDialogOpen, setGuestDialogOpen] = useState(false);

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
      const startTime = new Date(res.scanRegister2.period!.startTime! * 1000);
      dispatchTransaction({
        type: "PERSON_RESOLVED",
        uuid,
        periodId: res.scanRegister2.period!.id,
        person: res.scanRegister2.period!.person!,
        status: "SIGNED_IN",
        startTime,
      });
      return;
    } else if (state == "SIGN_OUT_PENDING") {
      audioSuccess.play();
      const startTime = new Date(res.scanRegister2.period!.startTime! * 1000);
      const quickPick = res.scanRegister2.quickPick;
      dispatchTransaction({
        type: "PERSON_RESOLVED",
        uuid,
        periodId: res.scanRegister2.period!.id,
        person: res.scanRegister2.period!.person!,
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

    dispatchTransaction({ type: "LOAD_PERSON", uuid, memberId });

    // purposefully not awaited - we want the form submission to be considered complete
    // so we can re-render
    completeSubmit({ memberId, uuid });
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

  function onResolveForgotSignOut(endTime: Date) {
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
    const onCompleted = () => {
      console.log("Adjust mutation completed");
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
        smallCategories={smallCategories}
        newCategories={newCategories}
      />
      <ScanScreenCategories
        screenPosition={categoriesPos}
        onSelectCategory={onSelectCategory}
        uuid={transactionUuid}
        smallCategories={smallCategories}
        newCategories={newCategories}
      />
      <ScanScreenAdjust
        screenPosition={adjustPos}
        onSubmit={onSubmitAdjust}
        onError={() => audioError.play()}
        uuid={transactionUuid}
        transaction={signedOutTransaction}
        onEditCategory={onEditCategory}
        isSubmitting={signOutIsInFlight}
        easyTimeEntry={easyTimeEntry}
        newCategories={newCategories}
      />
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
