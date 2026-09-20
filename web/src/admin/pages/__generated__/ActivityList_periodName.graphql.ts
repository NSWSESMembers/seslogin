/**
 * @generated SignedSource<<9178ba37857949a9af82d4a1d1b9f176>>
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
    readonly memberNumber: string | null | undefined;
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

(node as any).hash = "c877cf3bbb3eba31bce11bd90b1e6154";

export default node;
