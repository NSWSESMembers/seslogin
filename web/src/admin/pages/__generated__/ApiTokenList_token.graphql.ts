/**
 * @generated SignedSource<<e5fa42d00a3864db9c57f30c1555f4d2>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ApiTokenList_token$data = {
  readonly createdAt: number;
  readonly expiresAt: number | null | undefined;
  readonly id: string;
  readonly lastUsedAt: number | null | undefined;
  readonly locationGrants: ReadonlyArray<string>;
  readonly name: string;
  readonly readOnly: boolean;
  readonly " $fragmentType": "ApiTokenList_token";
};
export type ApiTokenList_token$key = {
  readonly " $data"?: ApiTokenList_token$data;
  readonly " $fragmentSpreads": FragmentRefs<"ApiTokenList_token">;
};

const node: ReaderFragment = {
  "argumentDefinitions": [],
  "kind": "Fragment",
  "metadata": {
    "throwOnFieldError": true
  },
  "name": "ApiTokenList_token",
  "selections": [
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "id",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "name",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "locationGrants",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "readOnly",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "createdAt",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "expiresAt",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "lastUsedAt",
      "storageKey": null
    }
  ],
  "type": "ApiToken",
  "abstractKey": null
};

(node as any).hash = "cfc0df976353f174b7256c9429a196c5";

export default node;
