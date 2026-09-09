import { useState } from "react";
import { graphql, useMutation } from "react-relay";
import type { CategoryListDisableMutation } from "./__generated__/CategoryListDisableMutation.graphql";
import type { CategoryListQuery } from "./__generated__/CategoryListQuery.graphql";
import { useUserInfo } from "../components/useUserInfo";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";
import { categories as kioskCategoryTree } from "../../lib/categories";

type CategoryData = {
  id: string;
  name: string;
  enabled: boolean;
  isVirtual: boolean;
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

// Only leaf (subcategory) ids from categories.ts ever get selected as a real
// category on the kiosk — the top-level entries (`C1`-`C10`) are pure
// navigation groupings and are never themselves a DB category id (see
// ScanScreenCategories: selecting one always drills into its subcategories
// rather than submitting it). So the leaves are what has to line up with the
// DB's enabled categories for the scan interface to work.
type KioskLeaf = { id: string; groupName: string; name: string };

const KIOSK_LEAVES: KioskLeaf[] = kioskCategoryTree.flatMap((top) =>
  (top.subcategories || []).map((sub) => ({
    id: sub.id,
    groupName: top.name,
    name: sub.name,
  })),
);
const KIOSK_LEAF_IDS = new Set(KIOSK_LEAVES.map((leaf) => leaf.id));

function Row({
  category,
  idx,
  isDev,
}: {
  category: CategoryData;
  idx: number;
  isDev: boolean;
}) {
  const inKioskList = KIOSK_LEAF_IDS.has(category.id);
  const missingFromKioskList = category.enabled && !inKioskList;
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] =
    useMutation<CategoryListDisableMutation>(graphql`
      mutation CategoryListDisableMutation(
        $id: ID!
        $name: String!
        $isVirtual: Boolean!
        $nitcGroupId: String
        $nitcParticipantType: String
      ) {
        updateCategory(
          id: $id
          name: $name
          enabled: false
          isVirtual: $isVirtual
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
              isVirtual: category.isVirtual,
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
    <tr
      className={
        missingFromKioskList
          ? "bg-red-100 dark:bg-red-950/50"
          : idx % 2 === 0
            ? "bg-surface-raised"
            : undefined
      }
    >
      {isDev && <Td className="font-mono text-[0.85em]">{category.id}</Td>}
      <Td nowrap>
        <div className={category.enabled ? undefined : "line-through"}>
          {category.name}
        </div>
      </Td>
      <Td>{category.isVirtual ? "Yes" : ""}</Td>
      <Td>{category.nitcParticipantType ?? ""}</Td>
      <Td className="font-mono text-[0.85em]">{category.nitcGroupId ?? ""}</Td>
      <Td>{category.nitcGroup?.nitcType ?? ""}</Td>
      <Td>{tagNames ?? ""}</Td>
      <Td>
        {missingFromKioskList ? (
          <span className="font-bold text-red-700 dark:text-red-400">
            ✗ Missing!
          </span>
        ) : inKioskList ? (
          "✓"
        ) : (
          ""
        )}
      </Td>
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

// A leaf entry from categories.ts that the scan interface can offer but that
// isn't a usable category in the DB right now — either there's no matching
// category at all, or there is one but it's disabled. Either way a member
// picking this on the kiosk would fail, so it's surfaced above the normal
// listing rather than mixed in (and shown regardless of "Show disabled").
function KioskOnlyRow({
  leaf,
  dbCategory,
  isDev,
}: {
  leaf: KioskLeaf;
  dbCategory: CategoryData | undefined;
  isDev: boolean;
}) {
  const reason = dbCategory ? "Disabled in DB" : "Not in DB";
  const tagNames = dbCategory?.nitcGroup?.sesTags.map((t) => t.name).join(", ");

  return (
    <tr className="bg-amber-100 dark:bg-amber-950/50">
      {isDev && (
        <Td className="font-mono text-[0.85em]">{dbCategory?.id ?? leaf.id}</Td>
      )}
      <Td nowrap>
        <span className="font-bold text-amber-800 dark:text-amber-300">
          {leaf.groupName} &gt; {leaf.name}
        </span>
      </Td>
      <Td>{dbCategory?.isVirtual ? "Yes" : ""}</Td>
      <Td>{dbCategory?.nitcParticipantType ?? ""}</Td>
      <Td className="font-mono text-[0.85em]">
        {dbCategory?.nitcGroupId ?? ""}
      </Td>
      <Td>{dbCategory?.nitcGroup?.nitcType ?? ""}</Td>
      <Td>{tagNames ?? ""}</Td>
      <Td>
        <span className="font-bold text-amber-800 dark:text-amber-300">
          ⚠ {reason}
        </span>
      </Td>
      <Td options>
        {dbCategory && (
          <div className="flex justify-end gap-1">
            <ButtonLink size="row" to={`/admin/categories/${dbCategory.id}`}>
              Edit
            </ButtonLink>
          </div>
        )}
      </Td>
    </tr>
  );
}

export default function CategoryList() {
  const { isDev } = useUserInfo();
  const [showDisabled, setShowDisabled] = useState(false);

  const data = useRetryableLazyLoadQuery<CategoryListQuery>(
    graphql`
      query CategoryListQuery @throwOnFieldError {
        categories {
          id
          name
          enabled
          isVirtual
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

  const dbCategoriesById = new Map(data.categories.map((c) => [c.id, c]));
  const kioskOnlyLeaves = KIOSK_LEAVES.filter((leaf) => {
    const dbCategory = dbCategoriesById.get(leaf.id);
    return !dbCategory || !dbCategory.enabled;
  }).sort((a, b) =>
    `${a.groupName} > ${a.name}`.localeCompare(`${b.groupName} > ${b.name}`),
  );

  return (
    <>
      <p>
        <span className="font-bold text-red-600 dark:text-red-400">
          Warning:
        </span>{" "}
        you must manually sync changes here to the dump of JSON categories that
        gets compiled into the JS frontend or else the listing of categories in
        the scan interface will not be updated.
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
            <Th>Virtual</Th>
            <Th>Participant Type</Th>
            <Th>NITC Group ID</Th>
            <Th>NITC Type</Th>
            <Th>SES Tags</Th>
            <Th>Scan Screen</Th>
            <Th style={{ width: 100 }}></Th>
          </tr>
        </thead>
        <tbody>
          {kioskOnlyLeaves.map((leaf) => (
            <KioskOnlyRow
              key={leaf.id}
              leaf={leaf}
              dbCategory={dbCategoriesById.get(leaf.id)}
              isDev={isDev}
            />
          ))}
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
