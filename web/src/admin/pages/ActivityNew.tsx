import { useState } from "react";
import { graphql } from "relay-runtime";
import { useLazyLoadQuery, useMutation } from "react-relay";
import { useNavigate } from "react-router";
import useSelectedLocation from "../components/useSelectedLocation";
import type { ActivityNewMutation } from "./__generated__/ActivityNewMutation.graphql";
import type { ActivityNewQuery } from "./__generated__/ActivityNewQuery.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import Select from "../../components/ui/Select";
import { Button } from "../../components/ui/Button";

export default function ActivityNew() {
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();
  const [startValue, setStartValue] = useState("");
  const [endValue, setEndValue] = useState("");
  const data = useLazyLoadQuery<ActivityNewQuery>(
    graphql`
      query ActivityNewQuery($location: ID!) {
        location(id: $location) {
          id
          people {
            id
            firstName
            lastName
          }
        }
        categories {
          id
          name
          enabled
        }
      }
    `,
    { location: locationId },
  );

  const [commitMutation, isMutationInFlight] = useMutation<ActivityNewMutation>(
    graphql`
      mutation ActivityNewMutation(
        $personId: ID!
        $locationId: ID!
        $startTime: Int!
        $endTime: Int!
        $categoryId: ID!
      ) {
        createPeriod(
          personId: $personId
          locationId: $locationId
          categoryId: $categoryId
          startTime: $startTime
          endTime: $endTime
        ) {
          id
        }
      }
    `,
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
    const personId = formData.get("person")?.toString() || "";
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
          variables: { personId, categoryId, startTime, endTime, locationId },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            const location = store.get(locationId);
            location?.invalidateRecord();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't create activity entry");
      return;
    }
    notifySuccess("Activity entry created");
    navigate("/admin/activity");
  }

  // only offer enabled categories, sorted alphabetically
  const categories = data.categories
    .filter((cat) => cat.enabled)
    .sort((a, b) => a.name.localeCompare(b.name));

  const people = [...data.location.people].sort((a, b) =>
    `${a.firstName} ${a.lastName}`.localeCompare(
      `${b.firstName} ${b.lastName}`,
    ),
  );

  return (
    <>
      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="person">Member</label>}>
            <Select name="person" id="person" required>
              {people.map((person) => (
                <option value={person.id} key={person.id}>
                  {person.firstName} {person.lastName}
                </option>
              ))}
            </Select>
          </FormField>
          <FormField label={<label htmlFor="category">Category</label>}>
            <Select name="category" id="category" required>
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
