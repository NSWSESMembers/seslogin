import { useState } from "react";
import { graphql } from "relay-runtime";
import { useLazyLoadQuery, useMutation } from "react-relay";
import { useNavigate } from "react-router";
import useSelectedLocation from "../components/useSelectedLocation";
import type { ActivityNewMutation } from "./__generated__/ActivityNewMutation.graphql";
import type { ActivityNewQuery } from "./__generated__/ActivityNewQuery.graphql";
import { useNotify } from "../components/useNotify";

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

  // sort categories alphabetically
  const categories = [...data.categories].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  const people = [...data.location.people].sort((a, b) =>
    `${a.firstName} ${a.lastName}`.localeCompare(
      `${b.firstName} ${b.lastName}`,
    ),
  );

  return (
    <>
      <form action={handleSubmit}>
        <dl>
          <dt>
            <label htmlFor="person" className="required">
              Member
            </label>
          </dt>
          <dd>
            <select name="person" id="person" required>
              {people.map((person) => (
                <option value={person.id} key={person.id}>
                  {person.firstName} {person.lastName}
                </option>
              ))}
            </select>
          </dd>
          <dt>
            <label htmlFor="category" className="required">
              Category
            </label>
          </dt>
          <dd>
            <select name="category" id="category" required>
              <option value="">-- Select category --</option>
              {categories.map((cat) => (
                <option value={cat.id} key={cat.id}>
                  {cat.name}
                </option>
              ))}
            </select>
          </dd>
          <dt>
            <label htmlFor="start" className="required">
              Start time
            </label>
          </dt>
          <dd>
            <input
              type="datetime-local"
              name="start"
              id="start"
              required
              value={startValue}
              onChange={(e) => setStartValue(e.target.value)}
            />
          </dd>
          <dt>
            <label htmlFor="end" className="required">
              End time
            </label>
          </dt>
          <dd>
            <input
              type="datetime-local"
              name="end"
              id="end"
              required
              value={endValue}
              onChange={(e) => setEndValue(e.target.value)}
            />
            {error && <p className="error">{error}</p>}
            {warning && <p className="warning">{warning}</p>}
          </dd>
          <dt>&nbsp;</dt>
          <dd>
            <button type="submit" disabled={isMutationInFlight || !!error}>
              Save
            </button>
          </dd>
        </dl>
      </form>
    </>
  );
}
