import { useState } from "react";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import { useNavigate, useParams } from "react-router";
import { dateToInputDateTimeLocal } from "../../lib/time";
import useSelectedLocation from "../components/useSelectedLocation";
import type { ActivityEditQuery } from "./__generated__/ActivityEditQuery.graphql";
import type { ActivityEditMutation } from "./__generated__/ActivityEditMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import Select from "../../components/ui/Select";
import { Button } from "../../components/ui/Button";

export default function ActivityEdit() {
  const params = useParams();
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const data = useLazyLoadQuery<ActivityEditQuery>(
    graphql`
      query ActivityEditQuery($id: ID!) {
        period(id: $id) {
          id
          startTime
          endTime
          category {
            id
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
    { id: params.periodId! },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<ActivityEditMutation>(graphql`
      mutation ActivityEditMutation(
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

  const [startValue, setStartValue] = useState(
    dateToInputDateTimeLocal(new Date(data.period.startTime * 1000)),
  );
  const [endValue, setEndValue] = useState(
    data.period.endTime
      ? dateToInputDateTimeLocal(new Date(data.period.endTime * 1000))
      : "",
  );

  const startMs = startValue ? new Date(startValue).getTime() : null;
  const endMs = endValue ? new Date(endValue).getTime() : null;
  let error: string | null = null;
  let warning: string | null = null;
  if (startMs !== null && endMs !== null) {
    if (startMs === endMs)
      error = "Start date must not be the same as end date";
    else if (endMs < startMs)
      error = "The end date must come after the start date";
    else if (endMs - startMs > 86400000)
      warning =
        "Warning: end date is more than 24h after start date - are you sure?";
  }

  async function handleSubmit(formData: FormData) {
    if (error) return;
    const categoryId = formData.get("category")?.toString() || "";
    const start = formData.get("start")?.toString();
    const end = formData.get("end")?.toString();
    if (!start) {
      notifyError("Start time is required");
      return;
    }
    if (!end) {
      notifyError("End time is required");
      return;
    }
    const startTime = Math.floor(new Date(start).getTime() / 1000);
    const endTime = Math.floor(new Date(end).getTime() / 1000);

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { id: data.period.id, startTime, endTime, categoryId },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            const location = store.get(locationId);
            location?.invalidateRecord();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't save activity entry");
      return;
    }
    notifySuccess("Activity entry saved");
    navigate("/admin/activity");
  }

  // only offer enabled categories, but keep this period's current category in
  // the list even if it has since been disabled, so editing times doesn't force
  // a category change. Sorted alphabetically.
  const currentCategoryId = data.period.category?.id;
  const categories = data.categories
    .filter((cat) => cat.enabled || cat.id === currentCategoryId)
    .sort((a, b) => a.name.localeCompare(b.name));

  return (
    <>
      <p>Edit the activity entry details, then click Save.</p>
      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="category">Category</label>}>
            <Select
              name="category"
              id="category"
              required
              defaultValue={data.period.category?.id || ""}
            >
              <option value="">-- Select category --</option>
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
            {warning && <p className="font-bold text-orange-600">{warning}</p>}
          </FormField>
          <FormField>
            <Button type="submit" disabled={isMutationInFlight || !!error}>
              Save
            </Button>
          </FormField>
        </FieldList>
      </form>
    </>
  );
}
