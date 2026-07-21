export type MemberIdWithUuid = {
  memberId: string;
  uuid: string;
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
  awardedBadges?: BadgeAward[];
};

export type BadgeAward = {
  id: string;
  name: string;
  description: string;
  tier: string;
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
  awardedBadges?: BadgeAward[];
};

export type PurgeExpiredTransactionsAction = {
  type: "PURGE_EXPIRED_TRANSACTIONS";
  now: Date;
};

export const FINALIZED_TRANSACTION_PURGE_AGE_MS = 60_000;

export type CancelTransactionAction = {
  type: "CANCEL_TRANSACTION";
  uuid: string;
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
  | CancelTransactionAction;

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
  awardedBadges?: BadgeAward[];
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
  awardedBadges?: BadgeAward[];
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
          awardedBadges: action.awardedBadges,
        };
      } else if (action.status == "SIGNED_OUT") {
        updatedTransaction = {
          uuid: action.uuid,
          person: action.person,
          startTime: action.startTime,
          endTime: action.endTime!,
          status: "SIGNED_OUT",
          adjusted: false,
          periodId: action.periodId,
          awardedBadges: action.awardedBadges,
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
        awardedBadges: action.awardedBadges ?? oldTransaction.awardedBadges,
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
