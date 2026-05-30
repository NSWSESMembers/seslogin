/**
 * @generated SignedSource<<b308da4e9d74b609a2ed47fe5fc27bd1>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ActivityListMember_periodName$data = {
  readonly location: {
    readonly id: string;
    readonly name: string;
  };
  readonly " $fragmentType": "ActivityListMember_periodName";
};
export type ActivityListMember_periodName$key = {
  readonly " $data"?: ActivityListMember_periodName$data;
  readonly " $fragmentSpreads": FragmentRefs<"ActivityListMember_periodName">;
};

const node: ReaderInlineDataFragment = {
  "kind": "InlineDataFragment",
  "name": "ActivityListMember_periodName"
};

(node as any).hash = "be7597eda0977db06bef5fd67a52182b";

export default node;
