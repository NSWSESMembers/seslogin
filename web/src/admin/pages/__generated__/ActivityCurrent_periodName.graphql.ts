/**
 * @generated SignedSource<<5383f8607c7f1c7729072fecb2a6be2c>>
 * @lightSyntaxTransform
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
    readonly memberNumber: string | null | undefined;
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

(node as any).hash = "5ad044ce7bcfe2303a7f25c88bc9fffd";

export default node;
