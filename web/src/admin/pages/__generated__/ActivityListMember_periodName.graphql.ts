/**
 * @generated SignedSource<<688fba5afdf3741fa863046ef52cfe97>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
import { FragmentRefs, Result } from "relay-runtime";
export type ActivityListMember_periodName$data = {
  readonly location: Result<{
    readonly id: string;
    readonly name: string;
  }, unknown>;
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

(node as any).hash = "ad1b147cd0e10cc7aad07c6e7c1eeee8";

export default node;
