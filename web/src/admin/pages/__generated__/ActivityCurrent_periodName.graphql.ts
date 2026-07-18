/**
 * @generated SignedSource<<61dbfa3b1efe2998fd02af9f626df1d8>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ActivityCurrent_periodName$data = {
  readonly guestName: string | null | undefined;
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

(node as any).hash = "d7c2268dcbddf883a38cc33f85e420be";

export default node;
