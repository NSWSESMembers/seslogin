import { useState } from "react";
import { Link } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { CategoryListDisableMutation } from "./__generated__/CategoryListDisableMutation.graphql";
import type { CategoryListQuery } from "./__generated__/CategoryListQuery.graphql";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";

type CategoryData = {
  id: string;
  name: string;
  enabled: boolean;
  nitcGroupId: string | null | undefined;
  nitcParticipantType: string | null | undefined;
  nitcGroup:
    | {
        id: string;
        nitcType: string;
        sesTags: ReadonlyArray<{ id: string; name: string }>;
      }
    | null
    | undefined;
};

function Row({
  category,
  idx,
  isDev,
}: {
  category: CategoryData;
  idx: number;
  isDev: boolean;
}) {
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] =
    useMutation<CategoryListDisableMutation>(graphql`
      mutation CategoryListDisableMutation(
        $id: ID!
        $name: String!
        $nitcGroupId: String
        $nitcParticipantType: String
      ) {
        updateCategory(
          id: $id
          name: $name
          enabled: false
          nitcGroupId: $nitcGroupId
          nitcParticipantType: $nitcParticipantType
        ) {
          id
          name
          enabled
        }
      }
    `);

  async function disableCategory() {
    const yes = confirm(
      `Are you sure you want to disable category ${category.name}?`,
    );
    if (yes) {
      try {
        await new Promise((resolve, reject) => {
          commitMutation({
            variables: {
              id: category.id,
              name: category.name,
              nitcGroupId: category.nitcGroupId ?? null,
              nitcParticipantType: category.nitcParticipantType ?? null,
            },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.invalidateStore();
            },
          });
        });
        notifySuccess(`Category ${category.name} disabled`);
      } catch (err) {
        notifyError(err, `Couldn't disable category ${category.name}`);
      }
    }
  }

  const tagNames = category.nitcGroup?.sesTags.map((t) => t.name).join(", ");

  return (
    <tr className={idx % 2 === 0 ? "odd" : "even"}>
      {isDev && (
        <td
          style={{ fontFamily: "monospace", fontSize: "0.85em" }}
          data-label="ID"
        >
          {category.id}
        </td>
      )}
      <td className="nowrap" data-label="Name">
        <div className={category.enabled ? "" : "strike"}>{category.name}</div>
      </td>
      <td data-label="Participant Type">
        {category.nitcParticipantType ?? ""}
      </td>
      <td
        style={{ fontFamily: "monospace", fontSize: "0.85em" }}
        data-label="NITC Group ID"
      >
        {category.nitcGroupId ?? ""}
      </td>
      <td data-label="NITC Type">{category.nitcGroup?.nitcType ?? ""}</td>
      <td data-label="SES Tags">{tagNames ?? ""}</td>
      <td className="options" data-label="Actions">
        <Link to={`/admin/categories/${category.id}`}>Edit</Link>&nbsp;
        {category.enabled && (
          <button
            className="delete"
            onClick={disableCategory}
            disabled={isMutationInFlight}
          >
            Disable
          </button>
        )}
      </td>
    </tr>
  );
}

export default function CategoryList() {
  const { isDev } = useUserInfo();
  const [showDisabled, setShowDisabled] = useState(false);

  const data = useLazyLoadQuery<CategoryListQuery>(
    graphql`
      query CategoryListQuery {
        categories {
          id
          name
          enabled
          nitcGroupId
          nitcParticipantType
          nitcGroup {
            id
            nitcType
            sesTags {
              id
              name
            }
          }
        }
      }
    `,
    {},
  );

  const categories = [...data.categories]
    .filter((c) => showDisabled || c.enabled)
    .sort((a, b) => a.name.localeCompare(b.name));

  return (
    <>
      <p>
        <span style={{ fontWeight: "bold", color: "red" }}>Warning:</span> you
        must manually sync changes here to the dump of JSON categories that gets
        compiled into the JS frontend or else the listing of categories in the
        scan interface will not be updated.
      </p>
      <p>
        <label>
          <input
            type="checkbox"
            checked={showDisabled}
            onChange={(e) => setShowDisabled(e.target.checked)}
          />{" "}
          Show disabled
        </label>
      </p>
      <table className="admin">
        <thead>
          <tr>
            {isDev && <th>ID</th>}
            <th>Name</th>
            <th>Participant Type</th>
            <th>NITC Group ID</th>
            <th>NITC Type</th>
            <th>SES Tags</th>
            <th style={{ width: 100 }}></th>
          </tr>
        </thead>
        <tbody>
          {categories.map((category, idx) => (
            <Row
              key={category.id}
              category={category}
              idx={idx}
              isDev={isDev}
            />
          ))}
        </tbody>
      </table>
    </>
  );
}
