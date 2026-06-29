import { Link } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { NitcGroupListQuery } from "./__generated__/NitcGroupListQuery.graphql";
import type { NitcGroupListDeleteMutation } from "./__generated__/NitcGroupListDeleteMutation.graphql";
import { useNotify } from "../components/useNotify";

type NitcGroupData = {
  id: string;
  nitcType: string;
  sesTags: ReadonlyArray<{ id: string; name: string }>;
};

type CategoryData = {
  id: string;
  name: string;
  nitcGroupId: string | null | undefined;
};

function Row({
  group,
  idx,
  categories,
}: {
  group: NitcGroupData;
  idx: number;
  categories: ReadonlyArray<CategoryData>;
}) {
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] =
    useMutation<NitcGroupListDeleteMutation>(graphql`
      mutation NitcGroupListDeleteMutation($id: ID!) {
        deleteNitcGroup(id: $id)
      }
    `);

  async function deleteGroup() {
    const yes = confirm(
      `Are you sure you want to delete NITC group ${group.id}?`,
    );
    if (yes) {
      try {
        await new Promise((resolve, reject) => {
          commitMutation({
            variables: { id: group.id },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.invalidateStore();
            },
          });
        });
        notifySuccess(`NITC group ${group.id} deleted`);
      } catch (err) {
        notifyError(err, `Couldn't delete NITC group ${group.id}`);
      }
    }
  }

  const tagNames = group.sesTags.map((t) => t.name).join(", ");
  const usingCategories = categories.filter((c) => c.nitcGroupId === group.id);
  const categoryNames = usingCategories.map((c) => c.name).join(", ");

  return (
    <tr className={idx % 2 === 0 ? "odd" : "even"}>
      <td style={{ fontFamily: "monospace", fontSize: "0.85em" }} data-label="ID">
        {group.id}
      </td>
      <td data-label="NITC Type">{group.nitcType}</td>
      <td data-label="SES Tags">{tagNames}</td>
      <td title={categoryNames || undefined} data-label="Categories">{usingCategories.length}</td>
      <td className="options" data-label="Actions">
        <Link to={`/admin/categories/nitc-groups/${group.id}`}>Edit</Link>&nbsp;
        <button
          className="delete"
          onClick={deleteGroup}
          disabled={isMutationInFlight}
        >
          Delete
        </button>
      </td>
    </tr>
  );
}

export default function NitcGroupList() {
  const data = useLazyLoadQuery<NitcGroupListQuery>(
    graphql`
      query NitcGroupListQuery {
        nitcGroups {
          id
          nitcType
          sesTags {
            id
            name
          }
        }
        categories {
          id
          name
          nitcGroupId
        }
      }
    `,
    {},
  );

  const groups = [...data.nitcGroups].sort((a, b) => a.id.localeCompare(b.id));

  return (
    <table className="admin">
      <thead>
        <tr>
          <th>ID</th>
          <th>NITC Type</th>
          <th>SES Tags</th>
          <th>Categories</th>
          <th style={{ width: 100 }}></th>
        </tr>
      </thead>
      <tbody>
        {groups.map((group, idx) => (
          <Row
            key={group.id}
            group={group}
            idx={idx}
            categories={data.categories}
          />
        ))}
      </tbody>
    </table>
  );
}
