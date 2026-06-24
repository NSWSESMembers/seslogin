/**
 * @generated SignedSource<<1ecfeed2ee95b7303189471fef001fda>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityNewQuery$variables = {
  location: string;
};
export type ActivityNewQuery$data = {
  readonly categories: ReadonlyArray<{
    readonly enabled: boolean;
    readonly id: string;
    readonly name: string;
  }>;
  readonly location: {
    readonly id: string;
    readonly people: ReadonlyArray<{
      readonly firstName: string;
      readonly id: string;
      readonly lastName: string;
    }>;
  };
};
export type ActivityNewQuery = {
  response: ActivityNewQuery$data;
  variables: ActivityNewQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "location"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "location"
      }
    ],
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "location",
    "plural": false,
    "selections": [
      (v1/*: any*/),
      {
        "alias": null,
        "args": null,
        "concreteType": "Person",
        "kind": "LinkedField",
        "name": "people",
        "plural": true,
        "selections": [
          (v1/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "firstName",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "lastName",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "concreteType": "Category",
    "kind": "LinkedField",
    "name": "categories",
    "plural": true,
    "selections": [
      (v1/*: any*/),
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
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityNewQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ActivityNewQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "226b5b487199dd718c1d59c579cd2fa7",
    "id": null,
    "metadata": {},
    "name": "ActivityNewQuery",
    "operationKind": "query",
    "text": "query ActivityNewQuery(\n  $location: ID!\n) {\n  location(id: $location) {\n    id\n    people {\n      id\n      firstName\n      lastName\n    }\n  }\n  categories {\n    id\n    name\n    enabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "ba41a784a1729b60124bec27511c0ba1";

export default node;
