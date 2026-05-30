/**
 * @generated SignedSource<<c0b5909425c44425e666b63e2ccfea6e>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ActivityCurrent_periodName$data = {
  readonly person: {
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
  };
  readonly " $fragmentType": "ActivityCurrent_periodName";
};
export type ActivityCurrent_periodName$key = {
  readonly " $data"?: ActivityCurrent_periodName$data;
  readonly " $fragmentSpreads": FragmentRefs<"ActivityCurrent_periodName">;
};

const node: ReaderInlineDataFragment = {
  "kind": "InlineDataFragment",
  "name": "ActivityCurrent_periodName"
};

(node as any).hash = "b1d28f3ff5cf89bcf0f52f1b3b45bd6f";

export default node;
