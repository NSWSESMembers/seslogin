import { useState } from "react";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { CategoryListDisableMutation } from "./__generated__/CategoryListDisableMutation.graphql";
import type { CategoryListQuery } from "./__generated__/CategoryListQuery.graphql";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";

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
    <tr className={idx % 2 === 0 ? "bg-neutral-50" : undefined}>
      {isDev && <Td className="font-mono text-[0.85em]">{category.id}</Td>}
      <Td nowrap>
        <div className={category.enabled ? undefined : "line-through"}>
          {category.name}
        </div>
      </Td>
      <Td>{category.nitcParticipantType ?? ""}</Td>
      <Td className="font-mono text-[0.85em]">{category.nitcGroupId ?? ""}</Td>
      <Td>{category.nitcGroup?.nitcType ?? ""}</Td>
      <Td>{tagNames ?? ""}</Td>
      <Td options>
        <div className="flex justify-end gap-1">
          <ButtonLink size="row" to={`/admin/categories/${category.id}`}>
            Edit
          </ButtonLink>
          {category.enabled && (
            <Button
              size="row"
              variant="danger"
              onClick={disableCategory}
              disabled={isMutationInFlight}
            >
              Disable
            </Button>
          )}
        </div>
      </Td>
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
      <AdminTable>
        <thead>
          <tr>
            {isDev && <Th>ID</Th>}
            <Th>Name</Th>
            <Th>Participant Type</Th>
            <Th>NITC Group ID</Th>
            <Th>NITC Type</Th>
            <Th>SES Tags</Th>
            <Th style={{ width: 100 }}></Th>
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
      </AdminTable>
    </>
  );
}
