/**
 * @generated SignedSource<<4d9143c94ed3c322e2f6483db134fcf0>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ActivityList_periodName$data = {
  readonly person: {
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
  };
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

(node as any).hash = "7f112403cf10080bc349d9c53364a50e";

export default node;
