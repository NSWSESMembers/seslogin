import { useState } from "react";
import { useNavigate } from "react-router";
import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { UserNewQuery } from "./__generated__/UserNewQuery.graphql";
import type { UserNewMutation } from "./__generated__/UserNewMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import Textarea from "../../components/ui/Textarea";
import { Muted } from "../../components/ui/Muted";
import { Button } from "../../components/ui/Button";
import LocationRolesField from "../components/LocationRolesField";
import {
  splitLocationRoles,
  type LocationRoleEntry,
} from "../components/locationRoles";
import { parseEmails } from "./userEmails";

export default function NewUser() {
  const navigate = useNavigate();
  const { notify, notifyError, notifySuccess } = useNotify();
  const data = useRetryableLazyLoadQuery<UserNewQuery>(
    graphql`
      query UserNewQuery @throwOnFieldError {
        locations {
          id
          name
        }
        users {
          email
        }
      }
    `,
    {},
  );

  const [commitMutation, isMutationInFlight] = useMutation<UserNewMutation>(
    graphql`
      mutation UserNewMutation(
        $email: String!
        $isSuper: Boolean!
        $locationGrants: [String!]!
        $readOnlyLocationGrants: [String!]!
      ) {
        createUser(
          email: $email
          isSuper: $isSuper
          locationGrants: $locationGrants
          readOnlyLocationGrants: $readOnlyLocationGrants
        ) {
          id
          email
        }
      }
    `,
  );

  const [roles, setRoles] = useState<LocationRoleEntry[]>([]);
  // Controlled so that, after a partial failure, it can be left holding just
  // the addresses that still need creating.
  const [emailText, setEmailText] = useState("");
  const [isCreating, setIsCreating] = useState(false);

  function createOne(
    email: string,
    isSuper: boolean,
    locationGrants: string[],
    readOnlyLocationGrants: string[],
  ) {
    return new Promise((resolve, reject) => {
      commitMutation({
        variables: { email, isSuper, locationGrants, readOnlyLocationGrants },
        onCompleted: resolve,
        onError: reject,
        updater: (store) => {
          store.invalidateStore();
        },
      });
    });
  }

  async function handleSubmit(formData: FormData) {
    const isSuper = formData.get("super") === "on";
    const { locationGrants, readOnlyLocationGrants } =
      splitLocationRoles(roles);

    const { emails, invalid } = parseEmails(emailText);
    if (invalid.length > 0) {
      notify(`Not a valid email: ${invalid.join(", ")}`);
      return;
    }
    if (emails.length === 0) {
      notify("Enter at least one email");
      return;
    }
    // Sign-in looks a user up by email, so a second record for the same
    // address would be ambiguous. Refuse the whole batch rather than create
    // some of it, so nothing has to be untangled afterwards.
    const existing = new Set(data.users.map((u) => u.email.toLowerCase()));
    const taken = emails.filter((e) => existing.has(e.toLowerCase()));
    if (taken.length > 0) {
      notify(`Already a user: ${taken.join(", ")}`);
      return;
    }

    // One at a time, so a failure part-way through leaves a clear split
    // between what was created and what wasn't.
    setIsCreating(true);
    const failed: string[] = [];
    for (const email of emails) {
      try {
        await createOne(email, isSuper, locationGrants, readOnlyLocationGrants);
      } catch (err) {
        notifyError(err, `Couldn't create ${email}`);
        failed.push(email);
      }
    }
    setIsCreating(false);

    const created = emails.length - failed.length;
    if (failed.length > 0) {
      if (created > 0) {
        notifySuccess(
          `Created ${created} of ${emails.length} users. The rest are left below to retry.`,
        );
      }
      setEmailText(failed.join("\n"));
      return;
    }

    notifySuccess(created === 1 ? "User created" : `${created} users created`);
    navigate("/admin/users");
  }

  const emailCount = parseEmails(emailText).emails.length;

  const locations = [...data.locations].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  return (
    <>
      <p className="my-4">
        Enter the details of the new user in the form below. To add several
        users with the same permissions at once, enter more than one email.
      </p>

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="email">Email</label>}>
            <Textarea
              name="email"
              id="email"
              rows={3}
              required
              autoComplete="off"
              spellCheck={false}
              value={emailText}
              onChange={(e) => setEmailText(e.target.value)}
              aria-describedby="email-hint"
            />
            <Muted id="email-hint" className="mt-1 text-sm">
              One or more, separated by commas or new lines. Everyone gets the
              permissions below.
            </Muted>
          </FormField>
          <FormField label={<label htmlFor="super">Super</label>}>
            <input type="checkbox" name="super" id="super" />
          </FormField>
          <FormField label={<label htmlFor="locations">Locations</label>}>
            <LocationRolesField
              id="locations"
              locations={locations.map((l) => ({ id: l.id, name: l.name }))}
              value={roles}
              onChange={setRoles}
            />
          </FormField>
          <FormField>
            <Button type="submit" disabled={isMutationInFlight || isCreating}>
              {emailCount > 1 ? `Create ${emailCount} users` : "Save"}
            </Button>
          </FormField>
        </FieldList>
      </form>
    </>
  );
}
