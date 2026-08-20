/**
 * @generated SignedSource<<56b3de3ec848c69202c913e46e09cc38>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type LocationListQuery$variables = Record<PropertyKey, never>;
export type LocationListQuery$data = {
  readonly locations: ReadonlyArray<{
    readonly enabled: boolean;
    readonly id: string;
    readonly name: string;
    readonly " $fragmentSpreads": FragmentRefs<"LocationList_item">;
  }>;
};
export type LocationListQuery = {
  response: LocationListQuery$data;
  variables: LocationListQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "enabled",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "LocationListQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "locations",
        "plural": true,
        "selections": [
          (v0/*: any*/),
          (v1/*: any*/),
          (v2/*: any*/),
          {
            "args": null,
            "kind": "FragmentSpread",
            "name": "LocationList_item"
          }
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "LocationListQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "locations",
        "plural": true,
        "selections": [
          (v0/*: any*/),
          (v1/*: any*/),
          (v2/*: any*/),
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
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "c33e467ae1eea1ad325cb771b79208dc",
    "id": null,
    "metadata": {},
    "name": "LocationListQuery",
    "operationKind": "query",
    "text": "query LocationListQuery {\n  locations {\n    id\n    name\n    enabled\n    ...LocationList_item\n  }\n}\n\nfragment LocationList_item on Location {\n  id\n  name\n  enabled\n  nitcEnabled\n  nitcCompleteOnExport\n  lastSuccessfulMemberSync\n}\n"
  }
};
})();

(node as any).hash = "4140795a8c340e0e869636840f3781e7";

export default node;
