import { graphql, useMutation } from "react-relay";
import type { TransactionSignedOut } from "../ScanState";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
} from "react";
import { reducer } from "../ScanState";
import ScanScreenCategories from "./ScanScreenCategories";
import ScanScreenMain from "./ScanScreenMain";
import ScanScreenAdjust from "./ScanScreenAdjust";
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

const PURGE_EXPIRED_TRANSACTIONS_INTERVAL_MS = 1_000;
const SCAN_TRANSACTION_LOG_LEASE_ID = "scan:transaction-log";

export default function ScanController(props: {
  onCancelSignOutChange?: (fn: (() => void) | null) => void;
  onSigningOutNameChange?: (name: string | null) => void;
}) {
  const session = useKioskSession();
  const smallCategories = !!session?.config?.smallCategories;

  const [transactionState, dispatchTransaction] = useReducer(reducer, {
    transactions: [],
  });
  const focusMainInputRef = useRef<(() => void) | null>(null);

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
      mutation ScanControllerRegister2Mutation($memberNumber: String!) {
        scanRegister2(memberNumber: $memberNumber) {
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

  async function completeSubmit(memberId: string, uuid: string) {
    focusMainInputRef.current?.();

    let res: ScanControllerRegister2Mutation$data;
    try {
      res = await new Promise((resolve, reject) => {
        commitRegister2Mutation({
          variables: { memberNumber: memberId },
          onCompleted: resolve,
          onError: reject,
        });
      });
    } catch (err) {
      console.error("Error during register2 mutation:", err);
      audioError.play();
      dispatchTransaction({
        type: "ERROR",
        uuid,
        message: "network issue while looking up member ID: " + memberId,
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
      dispatchTransaction({
        type: "PERSON_RESOLVED",
        uuid,
        periodId: res.scanRegister2.period!.id,
        person: res.scanRegister2.period!.person!,
        status: "SIGNED_OUT",
        startTime,
      });
      return;
    }

    console.log("Response:", res);
    throw new Error("Unknown scan state");
  }

  async function handleMemberIdEntered(memberId: string) {
    const uuid = crypto.randomUUID();

    dispatchTransaction({ type: "LOAD_PERSON", uuid, memberId });

    // purposefully not awaited - we want the form submission to be considered complete
    // so we can re-render
    completeSubmit(memberId, uuid);
  }

  // most recent transaction
  const newTransaction = transactionState.transactions[0];
  const memberIdEnabled = newTransaction?.status != "LOADING";
  const needsCategory =
    typeof newTransaction !== "undefined" &&
    newTransaction.status == "SIGNED_OUT" &&
    typeof newTransaction.categoryId === "undefined";
  const needsAdjust =
    typeof newTransaction !== "undefined" &&
    newTransaction.status == "SIGNED_OUT" &&
    !newTransaction.adjusted;
  const signedOutTransaction: TransactionSignedOut | null =
    newTransaction?.status === "SIGNED_OUT" ? newTransaction : null;

  // we use this as a key to ensure ScanCategories clears state for each transaction
  const transactionUuid =
    needsCategory || needsAdjust ? newTransaction.uuid : null;

  // Refs so onSubmitAdjust always reads latest values at call time regardless of memoization.
  // Synced in useLayoutEffect (not during render) to be safe under concurrent rendering.
  const signedOutTransactionRef = useRef(signedOutTransaction);
  const transactionUuidRef = useRef(transactionUuid);
  useLayoutEffect(() => {
    signedOutTransactionRef.current = signedOutTransaction;
    transactionUuidRef.current = transactionUuid;
  });

  const mainPos: ScreenPosition =
    needsCategory || needsAdjust ? "offLeft" : "center";
  const categoriesPos: ScreenPosition = needsCategory ? "center" : "offRight";
  const adjustPos: ScreenPosition =
    !needsCategory && needsAdjust ? "center" : "offRight";

  const onCancelSignOut = useCallback(() => {
    if (!transactionUuid) return;
    dispatchTransaction({ type: "CANCEL_TRANSACTION", uuid: transactionUuid });
    focusMainInputRef.current?.();
  }, [transactionUuid]);

  const canCancelSignOut = needsCategory || needsAdjust;
  const { onCancelSignOutChange } = props;
  useEffect(() => {
    onCancelSignOutChange?.(canCancelSignOut ? onCancelSignOut : null);
  }, [canCancelSignOut, onCancelSignOut, onCancelSignOutChange]);

  const signingOutName =
    (needsCategory || needsAdjust) && signedOutTransaction
      ? `${signedOutTransaction.person.firstName} ${signedOutTransaction.person.lastName}`
      : null;
  const { onSigningOutNameChange } = props;
  useEffect(() => {
    onSigningOutNameChange?.(signingOutName);
  }, [signingOutName, onSigningOutNameChange]);

  function onSelectCategory(uuid: string, categoryId: string) {
    dispatchTransaction({ type: "SET_CATEGORY", uuid, categoryId });
  }

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
      dispatchTransaction({
        type: "ERROR",
        uuid,
        message:
          "network issue while adjusting record for " +
          tx.person.firstName +
          " " +
          tx.person.lastName,
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
        onSubmit={handleMemberIdEntered}
        onFocusInputReady={(focusInput) => {
          focusMainInputRef.current = focusInput;
        }}
      />
      <ScanScreenCategories
        screenPosition={categoriesPos}
        onSelectCategory={onSelectCategory}
        uuid={transactionUuid}
        smallCategories={smallCategories}
      />
      <ScanScreenAdjust
        screenPosition={adjustPos}
        onSubmit={onSubmitAdjust}
        uuid={transactionUuid}
        transaction={signedOutTransaction}
        onEditCategory={onEditCategory}
        isSubmitting={signOutIsInFlight}
      />
    </>
  );
}
