/**
 * @generated SignedSource<<f7c333bf1c15762c39ca0bfb038158a8>>
 * @lightSyntaxTransform
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
  readonly sesApiHeadquartersId: string | null | undefined;
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
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "sesApiHeadquartersId",
      "storageKey": null
    }
  ],
  "type": "Location",
  "abstractKey": null
};

(node as any).hash = "7adc99bb1c389139c5e752d30a596e37";

export default node;
