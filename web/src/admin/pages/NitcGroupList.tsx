import { useRef, useState } from "react";
import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { NitcGroupListQuery } from "./__generated__/NitcGroupListQuery.graphql";
import type { NitcGroupListDeleteMutation } from "./__generated__/NitcGroupListDeleteMutation.graphql";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";
import { Popover } from "../../components/ui/Popover";

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

// The category-usage count as a trigger: the category names are on the `title` (hover)
// and in a popover on click, so it also works on touch — same pattern as CommentIndicator.
function CategoryCountHint({
  categories,
}: {
  categories: ReadonlyArray<CategoryData>;
}) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  if (categories.length === 0) {
    return <>0</>;
  }

  const names = categories.map((c) => c.name).join(", ");

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        title={names}
        aria-label={open ? "Hide categories" : "Show categories"}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="cursor-help underline decoration-dotted underline-offset-2"
      >
        {categories.length}
      </button>
      {open && (
        <Popover
          anchorRef={buttonRef}
          onDismiss={() => setOpen(false)}
          className="max-w-xs px-2 py-1.5 text-sm"
        >
          {names}
        </Popover>
      )}
    </>
  );
}

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

  return (
    <tr className={idx % 2 === 0 ? "bg-surface-raised" : undefined}>
      <Td className="font-mono text-sm">{group.id}</Td>
      <Td>{group.nitcType}</Td>
      <Td>{tagNames}</Td>
      <Td>
        <CategoryCountHint categories={usingCategories} />
      </Td>
      <Td options>
        <div className="flex justify-end gap-1">
          <ButtonLink
            size="row"
            to={`/admin/categories/nitc-groups/${group.id}`}
          >
            Edit
          </ButtonLink>
          <Button
            size="row"
            variant="danger"
            onClick={deleteGroup}
            disabled={isMutationInFlight}
          >
            Delete
          </Button>
        </div>
      </Td>
    </tr>
  );
}

export default function NitcGroupList() {
  const data = useRetryableLazyLoadQuery<NitcGroupListQuery>(
    graphql`
      query NitcGroupListQuery @throwOnFieldError {
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
    <AdminTable>
      <thead>
        <tr>
          <Th>ID</Th>
          <Th>NITC Type</Th>
          <Th>SES Tags</Th>
          <Th>Categories</Th>
          <Th style={{ width: 100 }}></Th>
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
    </AdminTable>
  );
}
