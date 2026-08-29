export type MemberIdWithUuid = {
  memberId: string;
  uuid: string;
};

export type QuickPickCategory = {
  categoryId: string;
  /**
   * First names of the people who most recently picked this category here. Only
   * carried on the location list — it's a "who else has been doing this" hint,
   * which makes no sense against your own history.
   */
  peopleNames?: string[];
};

/**
 * Sign-out category shortcuts, returned inline by the register mutation (see
 * `RegisterResult.quickPick`) rather than fetched separately when the sign-out
 * screen opens.
 */
export type QuickPickSuggestions = {
  location: QuickPickCategory[];
  person: QuickPickCategory[];
};

export type LoadPersonAction = MemberIdWithUuid & {
  type: "LOAD_PERSON";
};

export type PersonResolvedAction = {
  type: "PERSON_RESOLVED";
  uuid: string;
  status: "SIGNED_IN" | "SIGNED_OUT" | "ERROR";
  person: {
    id: string;
    firstName: string;
    lastName: string;
  };
  periodId: string;
  startTime: Date;
  endTime?: Date;
  /** Only present when signing out, and only if the kiosk asked for it. */
  quickPick?: QuickPickSuggestions;
};

export type ErrorAction = {
  type: "ERROR";
  uuid: string;
  message: string;
};

export type AbortAction = {
  type: "ABORT";
  uuid: undefined;
  message: string;
};

export type SetCategoryAction = {
  type: "SET_CATEGORY";
  uuid: string;
  categoryId: string;
};

export type ClearCategoryAction = {
  type: "CLEAR_CATEGORY";
  uuid: string;
};

export type AdjustPeriodAction = {
  type: "ADJUST_PERIOD";
  uuid: string;
  startTime: Date;
  endTime: Date;
};

export type PurgeExpiredTransactionsAction = {
  type: "PURGE_EXPIRED_TRANSACTIONS";
  now: Date;
};

export const FINALIZED_TRANSACTION_PURGE_AGE_MS = 60_000;

/**
 * Signed in for longer than this when scanning to sign out → the kiosk shows the
 * "did you forget to sign out?" interstitial before the category screen.
 */
export const FORGOT_SIGN_OUT_PROMPT_THRESHOLD_MS = 12 * 60 * 60 * 1000;

export type CancelTransactionAction = {
  type: "CANCEL_TRANSACTION";
  uuid: string;
};

export type SkipQuickPickAction = {
  type: "SKIP_QUICK_PICK";
  uuid: string;
};

/**
 * Answer to the "you may have forgotten to sign out" interstitial: stamp the
 * chosen end time on the period and mark the prompt as dealt with so the flow
 * moves on to category selection.
 */
export type ResolveForgotSignOutAction = {
  type: "RESOLVE_FORGOT_SIGN_OUT";
  uuid: string;
  endTime: Date;
};

export type TransactionAction =
  | LoadPersonAction
  | PersonResolvedAction
  | ErrorAction
  | AbortAction
  | SetCategoryAction
  | ClearCategoryAction
  | AdjustPeriodAction
  | PurgeExpiredTransactionsAction
  | CancelTransactionAction
  | SkipQuickPickAction
  | ResolveForgotSignOutAction;

export type TransactionSignedIn = {
  uuid: string;
  status: "SIGNED_IN";
  finalizedTime: Date;
  periodId: string;
  person: {
    id: string;
    firstName: string;
    lastName: string;
  };
  startTime: Date;
};

export type TransactionSignedOut = {
  uuid: string;
  status: "SIGNED_OUT";
  finalizedTime?: Date;
  periodId: string;
  person: {
    id: string;
    firstName: string;
    lastName: string;
  };
  startTime: Date;
  endTime?: Date;
  categoryId?: string;
  adjusted: boolean;
  quickPickSkipped: boolean;
  /**
   * Whether the person had been signed in implausibly long (see
   * `FORGOT_SIGN_OUT_PROMPT_THRESHOLD_MS`) when they scanned to sign out —
   * decided once, when the transaction resolves.
   */
  longSession: boolean;
  /**
   * Set once the "you may have forgotten to sign out" interstitial has been
   * answered, so it isn't shown again for this transaction.
   */
  forgotSignOutPrompted: boolean;
  quickPick?: QuickPickSuggestions;
};

export type TransactionLoading = MemberIdWithUuid & {
  status: "LOADING";
};

export type TransactionAborted = {
  uuid: undefined;
  status: "ERROR";
  finalizedTime: Date;
  message: string;
};

export type TransactionError = {
  uuid: string;
  status: "ERROR";
  finalizedTime: Date;
  message: string;
};

export type Transaction =
  | TransactionSignedIn
  | TransactionSignedOut
  | TransactionLoading
  | TransactionError
  | TransactionAborted;

export type TransactionState = {
  transactions: Transaction[];
};

export function reducer(
  state: TransactionState,
  action: TransactionAction,
): TransactionState {
  switch (action.type) {
    case "LOAD_PERSON": {
      const newTransaction: TransactionLoading = {
        uuid: action.uuid,
        memberId: action.memberId,
        status: "LOADING",
      };
      return {
        ...state,
        transactions: [newTransaction, ...state.transactions],
      };
    }
    case "PERSON_RESOLVED": {
      const idx = state.transactions.findIndex((t) => t.uuid === action.uuid);
      if (idx === -1) {
        throw Error(
          "Could not find transaction while resolving uuid " + action.uuid,
        );
      }
      const finalizedTime = new Date();
      let updatedTransaction: Transaction;
      if (action.status == "SIGNED_IN") {
        updatedTransaction = {
          uuid: action.uuid,
          person: action.person,
          startTime: action.startTime,
          finalizedTime,
          status: "SIGNED_IN",
          periodId: action.periodId,
        };
      } else if (action.status == "SIGNED_OUT") {
        updatedTransaction = {
          uuid: action.uuid,
          person: action.person,
          startTime: action.startTime,
          endTime: action.endTime!,
          status: "SIGNED_OUT",
          adjusted: false,
          quickPickSkipped: false,
          longSession:
            Date.now() - action.startTime.getTime() >
            FORGOT_SIGN_OUT_PROMPT_THRESHOLD_MS,
          forgotSignOutPrompted: false,
          periodId: action.periodId,
          quickPick: action.quickPick,
        };
      } else {
        throw Error("Invalid status in PERSON_RESOLVED action");
      }
      return {
        ...state,
        transactions: [
          ...state.transactions.slice(0, idx),
          updatedTransaction,
          ...state.transactions.slice(idx + 1),
        ],
      };
    }
    case "ERROR": {
      const errorTransaction: TransactionError = {
        uuid: action.uuid,
        status: "ERROR",
        finalizedTime: new Date(),
        message: action.message,
      };

      const idx = state.transactions.findIndex((t) => t.uuid === action.uuid);
      if (idx === -1) {
        throw Error("Could not find transaction for error uuid " + action.uuid);
      }

      return {
        ...state,
        transactions: [
          ...state.transactions.slice(0, idx),
          errorTransaction,
          ...state.transactions.slice(idx + 1),
        ],
      };
    }
    case "ABORT": {
      const transactionAborted: TransactionAborted = {
        uuid: action.uuid,
        status: "ERROR",
        finalizedTime: new Date(),
        message: action.message,
      };
      return {
        ...state,
        transactions: [transactionAborted, ...state.transactions],
      };
    }
    case "SET_CATEGORY": {
      const idx = state.transactions.findIndex((t) => t.uuid === action.uuid);
      if (idx === -1) {
        throw Error(
          "Could not find transaction while resolving uuid " + action.uuid,
        );
      }
      const oldTransaction = state.transactions[idx];
      if (oldTransaction.status != "SIGNED_OUT") {
        throw Error(
          "Doesn't make sense to update category of transaction not in SIGNED_OUT state",
        );
      }
      const updatedTransaction: TransactionSignedOut = {
        ...oldTransaction,
        categoryId: action.categoryId,
      };
      return {
        ...state,
        transactions: [
          ...state.transactions.slice(0, idx),
          updatedTransaction,
          ...state.transactions.slice(idx + 1),
        ],
      };
    }
    case "CLEAR_CATEGORY": {
      const idx = state.transactions.findIndex((t) => t.uuid === action.uuid);
      if (idx === -1) {
        throw Error(
          "Could not find transaction while resolving uuid " + action.uuid,
        );
      }
      const oldTransaction = state.transactions[idx];
      if (oldTransaction.status != "SIGNED_OUT") {
        throw Error(
          "Doesn't make sense to clear category of transaction not in SIGNED_OUT state",
        );
      }
      const updatedTransaction: TransactionSignedOut = {
        ...oldTransaction,
      };
      delete updatedTransaction.categoryId;
      return {
        ...state,
        transactions: [
          ...state.transactions.slice(0, idx),
          updatedTransaction,
          ...state.transactions.slice(idx + 1),
        ],
      };
    }
    case "SKIP_QUICK_PICK": {
      const idx = state.transactions.findIndex((t) => t.uuid === action.uuid);
      if (idx === -1) {
        throw Error(
          "Could not find transaction while resolving uuid " + action.uuid,
        );
      }
      const oldTransaction = state.transactions[idx];
      if (oldTransaction.status != "SIGNED_OUT") {
        throw Error(
          "Doesn't make sense to skip quick pick for transaction not in SIGNED_OUT state",
        );
      }
      const updatedTransaction: TransactionSignedOut = {
        ...oldTransaction,
        quickPickSkipped: true,
      };
      return {
        ...state,
        transactions: [
          ...state.transactions.slice(0, idx),
          updatedTransaction,
          ...state.transactions.slice(idx + 1),
        ],
      };
    }
    case "RESOLVE_FORGOT_SIGN_OUT": {
      const idx = state.transactions.findIndex((t) => t.uuid === action.uuid);
      if (idx === -1) {
        throw Error(
          "Could not find transaction while resolving uuid " + action.uuid,
        );
      }
      const oldTransaction = state.transactions[idx];
      if (oldTransaction.status != "SIGNED_OUT") {
        throw Error(
          "Doesn't make sense to resolve a forgotten sign-out for transaction not in SIGNED_OUT state",
        );
      }
      const updatedTransaction: TransactionSignedOut = {
        ...oldTransaction,
        endTime: action.endTime,
        forgotSignOutPrompted: true,
      };
      return {
        ...state,
        transactions: [
          ...state.transactions.slice(0, idx),
          updatedTransaction,
          ...state.transactions.slice(idx + 1),
        ],
      };
    }
    case "ADJUST_PERIOD": {
      const idx = state.transactions.findIndex((t) => t.uuid === action.uuid);
      if (idx === -1) {
        throw Error(
          "Could not find transaction while resolving uuid " + action.uuid,
        );
      }
      const oldTransaction = state.transactions[idx];
      if (oldTransaction.status != "SIGNED_OUT") {
        throw Error(
          "Doesn't make sense to amend period of transaction not in SIGNED_OUT state",
        );
      }
      const updatedTransaction: TransactionSignedOut = {
        ...oldTransaction,
        startTime: action.startTime,
        endTime: action.endTime,
        adjusted: true,
        finalizedTime: new Date(),
      };
      return {
        ...state,
        transactions: [
          ...state.transactions.slice(0, idx),
          updatedTransaction,
          ...state.transactions.slice(idx + 1),
        ],
      };
    }
    case "PURGE_EXPIRED_TRANSACTIONS": {
      return {
        ...state,
        transactions: state.transactions.filter((t) => {
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
          return (
            action.now.getTime() - t.finalizedTime.getTime() <=
            FINALIZED_TRANSACTION_PURGE_AGE_MS
          );
        }),
      };
    }
    case "CANCEL_TRANSACTION": {
      return {
        ...state,
        transactions: state.transactions.filter((t) => t.uuid !== action.uuid),
      };
    }
    default: {
      throw Error("Unknown action: " + action["type"]);
    }
  }
}
