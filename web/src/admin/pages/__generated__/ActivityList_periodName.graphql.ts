/**
 * @generated SignedSource<<e37cbffe599ba2cdd023e0f8b167a0a0>>
 * @lightSyntaxTransform
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
    readonly isMemberOf: boolean;
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

(node as any).hash = "5b40681aba8183bd2bca0c506cda72f7";

export default node;
