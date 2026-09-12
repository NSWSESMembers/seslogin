/**
 * @generated SignedSource<<ee3242f52a0969e99e8d74e493a87e65>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
import { FragmentRefs, Result } from "relay-runtime";
export type ActivityCurrent_periodName$data = {
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

(node as any).hash = "f57cd236318d9873f5c5322f724f00ec";

export default node;
