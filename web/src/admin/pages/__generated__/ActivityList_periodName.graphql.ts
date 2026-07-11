/**
 * @generated SignedSource<<761a8eeb60a45270ad69707ed6231b62>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ActivityList_periodName$data = {
  readonly guestName: string | null | undefined;
  readonly person: {
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
  } | null | undefined;
  readonly " $fragmentType": "ActivityList_periodName";
};
export type ActivityList_periodName$key = {
  readonly " $data"?: ActivityList_periodName$data;
  readonly " $fragmentSpreads": FragmentRefs<"ActivityList_periodName">;
};

const node: ReaderInlineDataFragment = {
  "kind": "InlineDataFragment",
  "name": "ActivityList_periodName"
};

(node as any).hash = "2903d0da1af09a2c3fb6bbf5665426de";

export default node;
