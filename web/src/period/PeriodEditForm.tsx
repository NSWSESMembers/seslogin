import { useState } from "react";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";

import { dateToInputDateTimeLocal } from "../lib/time";
import { getErrorMessage } from "../lib/relayErrors";
import { FieldList, FormField } from "../components/ui/FormField";
import TextInput from "../components/ui/TextInput";
import Select from "../components/ui/Select";
import { Button } from "../components/ui/Button";
import {
  Panel,
  PanelBox,
  PanelIntro,
  PanelMessage,
  PanelTitle,
} from "../components/ui/Panel";
import PeriodEditConfirmation from "./PeriodEditConfirmation";
import type { PeriodEditFormQuery } from "./__generated__/PeriodEditFormQuery.graphql";
import type { PeriodEditFormMutation } from "./__generated__/PeriodEditFormMutation.graphql";

/** What was actually saved, for the confirmation screen. */
export type SavedEntry = {
  startTime: number;
  endTime: number;
  categoryName: string;
  locationName: string;
};

export default function PeriodEditForm() {
  // `linkedPeriod` takes no id: the token in the Authorization header is the
  // scope, so this can only ever return the one period the link was issued for.
  const data = useLazyLoadQuery<PeriodEditFormQuery>(
    graphql`
      query PeriodEditFormQuery @throwOnFieldError {
        linkedPeriod {
          id
          startTime
          endTime
          category {
            id
            name
          }
          person {
            firstName
            lastName
          }
          location {
            name
          }
        }
        categories {
          id
          name
          enabled
        }
      }
    `,
    {},
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<PeriodEditFormMutation>(graphql`
      mutation PeriodEditFormMutation(
        $id: ID!
        $startTime: Int!
        $endTime: Int!
        $categoryId: ID!
      ) {
        updatePeriodTimeCategory(
          id: $id
          startTime: $startTime
          endTime: $endTime
          categoryId: $categoryId
        ) {
          id
          startTime
          endTime
          category {
            id
            name
          }
        }
      }
    `);

  const period = data.linkedPeriod;

  const [startValue, setStartValue] = useState(
    dateToInputDateTimeLocal(new Date(period.startTime * 1000)),
  );
  const [endValue, setEndValue] = useState(
    period.endTime
      ? dateToInputDateTimeLocal(new Date(period.endTime * 1000))
      : "",
  );
  const [categoryValue, setCategoryValue] = useState(period.category?.id ?? "");
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [saved, setSaved] = useState<SavedEntry | null>(null);

  const startMs = startValue ? new Date(startValue).getTime() : null;
  const endMs = endValue ? new Date(endValue).getTime() : null;
  let error: string | null = null;
  let warning: string | null = null;
  if (startMs !== null && endMs !== null) {
    if (startMs === endMs)
      error = "Start time must not be the same as end time";
    else if (endMs < startMs)
      error = "The end time must come after the start time";
    // The server rejects anything over 24h for a link edit, so this is an error
    // here rather than the admin form's warning.
    else if (endMs - startMs > 86400000)
      error = "A time entry can't be longer than 24 hours";
    else if (endMs - startMs > 12 * 3600000)
      warning = "That's more than 12 hours — is that right?";
  }

  // Only offer enabled categories, but keep this entry's current one in the list
  // even if it has since been retired, so correcting a time doesn't force a
  // category change. Sorted alphabetically.
  const currentCategoryId = period.category?.id;
  const categories = data.categories
    .filter((cat) => cat.enabled || cat.id === currentCategoryId)
    .sort((a, b) => a.name.localeCompare(b.name));

  async function handleSubmit(formData: FormData) {
    if (error) return;
    setSubmitError(null);

    const categoryId = formData.get("category")?.toString() || "";
    const start = formData.get("start")?.toString();
    const end = formData.get("end")?.toString();
    if (!start || !end) {
      setSubmitError("Both a start time and an end time are required");
      return;
    }
    const startTime = Math.floor(new Date(start).getTime() / 1000);
    const endTime = Math.floor(new Date(end).getTime() / 1000);

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { id: period.id, startTime, endTime, categoryId },
          onCompleted: resolve,
          onError: reject,
        });
      });
    } catch (err) {
      setSubmitError(getErrorMessage(err));
      return;
    }

    setSaved({
      startTime,
      endTime,
      categoryName:
        categories.find((cat) => cat.id === categoryId)?.name ?? "Unknown",
      // The link can't move an entry between units, so the location the page
      // loaded with is still the one it was saved against.
      locationName: period.location.name,
    });
  }

  if (saved) {
    return (
      <PeriodEditConfirmation
        saved={saved}
        onChangeAgain={() => setSaved(null)}
      />
    );
  }

  const name = period.person
    ? `${period.person.firstName} ${period.person.lastName}`.trim()
    : null;

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Check your time entry</PanelTitle>
        <PanelIntro>
          {name ? `${name}, if ` : "If "}the times or activity below aren't
          right, correct them and save. Recorded at {period.location.name}.
        </PanelIntro>

        {submitError && <PanelMessage>{submitError}</PanelMessage>}

        <form action={handleSubmit}>
          <FieldList>
            <FormField label={<label htmlFor="category">Activity</label>}>
              <Select
                name="category"
                id="category"
                required
                value={categoryValue}
                onChange={(e) => setCategoryValue(e.target.value)}
              >
                <option value="">-- Select activity --</option>
                {categories.map((cat) => (
                  <option value={cat.id} key={cat.id}>
                    {cat.name}
                  </option>
                ))}
              </Select>
            </FormField>
            <FormField label={<label htmlFor="start">Start time</label>}>
              <TextInput
                type="datetime-local"
                name="start"
                id="start"
                required
                value={startValue}
                onChange={(e) => setStartValue(e.target.value)}
              />
            </FormField>
            <FormField label={<label htmlFor="end">End time</label>}>
              <TextInput
                type="datetime-local"
                name="end"
                id="end"
                required
                value={endValue}
                onChange={(e) => setEndValue(e.target.value)}
              />
              {error && <p className="font-bold text-red-600">{error}</p>}
              {warning && (
                <p className="font-bold text-orange-600">{warning}</p>
              )}
            </FormField>
            <FormField>
              <Button type="submit" disabled={isMutationInFlight || !!error}>
                {isMutationInFlight ? "Saving…" : "Save"}
              </Button>
            </FormField>
          </FieldList>
        </form>
      </PanelBox>
    </Panel>
  );
}
