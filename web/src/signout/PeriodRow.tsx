import { useMemo, useState } from "react";
import { graphql, useMutation } from "react-relay";
import type { PeriodRowMutation } from "./__generated__/PeriodRowMutation.graphql";
import {
  formatDayDateTime,
  formatTimeDiff,
  dateToInputDateTimeLocal,
} from "../lib/time";
import {
  groupCategories,
  categoryIconSrc,
  type CategoryGroup,
} from "./categoryIcon";

interface Category {
  id: string;
  name: string;
}

interface PeriodRowProps {
  personId: string;
  periodId: string;
  locationName: string;
  startTime: number;
  categories: ReadonlyArray<Category>;
  onSignedOut: () => void;
}

type Step = "idle" | "categoryGroup" | "categoryOption" | "confirmTimes";

export default function PeriodRow({
  personId,
  periodId,
  locationName,
  startTime,
  categories,
  onSignedOut,
}: PeriodRowProps) {
  const groups = useMemo(() => groupCategories(categories), [categories]);
  const [step, setStep] = useState<Step>("idle");
  const [activeGroup, setActiveGroup] = useState<CategoryGroup | null>(null);
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [startInput, setStartInput] = useState(() =>
    dateToInputDateTimeLocal(new Date(startTime * 1000)),
  );
  const [endInput, setEndInput] = useState(() =>
    dateToInputDateTimeLocal(new Date()),
  );
  const [error, setError] = useState<string | null>(null);
  const [commitMutation, isInFlight] = useMutation<PeriodRowMutation>(graphql`
    mutation PeriodRowMutation(
      $personId: ID!
      $periodId: ID!
      $categoryId: ID!
      $startTime: Int!
      $endTime: Int!
    ) {
      memberSignOut(
        personId: $personId
        periodId: $periodId
        categoryId: $categoryId
        startTime: $startTime
        endTime: $endTime
      ) {
        id
      }
    }
  `);

  const start = new Date(startTime * 1000);

  function handleSelectGroup(group: CategoryGroup) {
    setActiveGroup(group);
    setStep("categoryOption");
  }

  function handleSelectCategory(id: string) {
    setCategoryId(id);
    setError(null);
    setStep("confirmTimes");
  }

  function handleConfirm() {
    const startDate = new Date(startInput);
    const endDate = new Date(endInput);
    if (!categoryId) return;
    if (startDate.getTime() >= endDate.getTime()) {
      setError("Start time must be before end time.");
      return;
    }
    setError(null);
    commitMutation({
      variables: {
        personId,
        periodId,
        categoryId,
        startTime: Math.floor(startDate.getTime() / 1000),
        endTime: Math.floor(endDate.getTime() / 1000),
      },
      onCompleted: () => onSignedOut(),
      onError: () => {
        setError("Couldn't sign out of this session. Please try again.");
      },
    });
  }

  function handleCancel() {
    setStep("idle");
    setActiveGroup(null);
    setCategoryId(null);
    setError(null);
  }

  return (
    <li className="signout-list__item">
      <div className="signout-list__info">
        <div className="signout-list__location">{locationName}</div>
        <div className="signout-list__time">
          Signed in {formatDayDateTime(start)} &middot;{" "}
          {formatTimeDiff(start, new Date())} ago
        </div>
      </div>

      {step === "idle" && (
        <button
          type="button"
          className="action-button action-panel__button"
          onClick={() => setStep("categoryGroup")}
        >
          Sign out
        </button>
      )}

      {step === "categoryGroup" && (
        <div className="signout-list__categories">
          <p className="signout-list__prompt">
            What were you doing at this session?
          </p>
          <ul className="signout-list__category-options">
            {groups.map((group) => (
              <li key={group.id}>
                <button
                  type="button"
                  className="action-button signout-list__category-option"
                  onClick={() => handleSelectGroup(group)}
                >
                  {group.icon && (
                    <img
                      className="signout-list__category-icon"
                      src={categoryIconSrc(group.icon)}
                      alt=""
                    />
                  )}
                  {group.name}
                </button>
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="action-button action-panel__button action-panel__button--secondary"
            onClick={handleCancel}
          >
            Cancel
          </button>
        </div>
      )}

      {step === "categoryOption" && activeGroup && (
        <div className="signout-list__categories">
          <p className="signout-list__prompt">{activeGroup.name}</p>
          <ul className="signout-list__category-options">
            {activeGroup.options.map((option) => (
              <li key={option.id}>
                <button
                  type="button"
                  className="action-button signout-list__category-option"
                  onClick={() => handleSelectCategory(option.id)}
                >
                  {option.icon && (
                    <img
                      className="signout-list__category-icon"
                      src={categoryIconSrc(option.icon)}
                      alt=""
                    />
                  )}
                  {option.name}
                </button>
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="action-button action-panel__button action-panel__button--secondary"
            onClick={() => setStep("categoryGroup")}
          >
            Back
          </button>
        </div>
      )}

      {step === "confirmTimes" && (
        <div className="signout-list__confirm-times">
          <p className="signout-list__prompt">Confirm your times</p>
          <label className="signout-list__time-field">
            Start time
            <input
              type="datetime-local"
              className="action-panel__input"
              value={startInput}
              onChange={(e) => setStartInput(e.target.value)}
              disabled={isInFlight}
            />
          </label>
          <label className="signout-list__time-field">
            End time
            <input
              type="datetime-local"
              className="action-panel__input"
              value={endInput}
              onChange={(e) => setEndInput(e.target.value)}
              disabled={isInFlight}
            />
          </label>

          {error && (
            <div className="action-panel__message action-panel__message--error">
              {error}
            </div>
          )}

          <button
            type="button"
            className="action-button action-panel__button"
            onClick={handleConfirm}
            disabled={isInFlight}
          >
            {isInFlight ? "Signing out..." : "Confirm sign out"}
          </button>
          <button
            type="button"
            className="action-button action-panel__button action-panel__button--secondary"
            onClick={() => setStep("categoryOption")}
            disabled={isInFlight}
          >
            Back
          </button>
        </div>
      )}
    </li>
  );
}
