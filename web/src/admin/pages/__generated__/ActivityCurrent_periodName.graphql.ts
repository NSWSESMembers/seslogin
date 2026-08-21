/**
 * @generated SignedSource<<f0328d20c15360225ebe53f4af88f0e1>>
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

(node as any).hash = "5ec07e853527fdbf8c003b654a9aa2c3";

export default node;
