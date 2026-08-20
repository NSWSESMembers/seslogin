/**
 * @generated SignedSource<<16b505c02960ca560b1e25aa83d037ce>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type LocationList_item$data = {
  readonly enabled: boolean;
  readonly id: string;
  readonly lastSuccessfulMemberSync: number | null | undefined;
  readonly name: string;
  readonly nitcCompleteOnExport: boolean;
  readonly nitcEnabled: number | null | undefined;
  readonly " $fragmentType": "LocationList_item";
};
export type LocationList_item$key = {
  readonly " $data"?: LocationList_item$data;
  readonly " $fragmentSpreads": FragmentRefs<"LocationList_item">;
};

const node: ReaderFragment = {
  "argumentDefinitions": [],
  "kind": "Fragment",
  "metadata": {
    "throwOnFieldError": true
  },
  "name": "LocationList_item",
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
      "name": "enabled",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "nitcEnabled",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "nitcCompleteOnExport",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "lastSuccessfulMemberSync",
      "storageKey": null
    }
  ],
  "type": "Location",
  "abstractKey": null
};

(node as any).hash = "f20a9d6035256997c2e79e5fea80577d";

export default node;
