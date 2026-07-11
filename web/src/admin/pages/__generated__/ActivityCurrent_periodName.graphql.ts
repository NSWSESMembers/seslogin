/**
 * @generated SignedSource<<b1b1e06c8ceee1303cffd7b84c14c252>>
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
  } | null | undefined;
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
