/**
 * @generated SignedSource<<92e1577422d06e6976c01d2c8bfd2061>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
import { FragmentRefs, Result } from "relay-runtime";
export type ActivityList_periodName$data = {
  readonly guestName: string | null | undefined;
  readonly person: Result<{
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
    readonly location: {
      readonly id: string;
      readonly name: string;
    };
  } | null | undefined, unknown>;
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

(node as any).hash = "0d7b70a90a3b3fa6838714cc953b37fa";

export default node;
