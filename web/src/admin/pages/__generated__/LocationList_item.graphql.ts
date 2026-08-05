/**
 * @generated SignedSource<<59318e7f41adcce378477e13683a586d>>
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
  "metadata": null,
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

(node as any).hash = "a0d40c8441c0fbc9a538596b75915e17";

export default node;
