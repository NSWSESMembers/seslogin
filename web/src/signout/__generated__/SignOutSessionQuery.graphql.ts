/**
 * @generated SignedSource<<ea81e34f02498a823f635f5d2c9d6a27>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SignOutSessionQuery$variables = {
  personId: string;
};
export type SignOutSessionQuery$data = {
  readonly personSignoutSession: {
    readonly categories: ReadonlyArray<{
      readonly id: string;
      readonly name: string;
    }>;
    readonly firstName: string;
    readonly openPeriods: ReadonlyArray<{
      readonly id: string;
      readonly location: {
        readonly name: string;
      };
      readonly startTime: number;
    }>;
  } | null | undefined;
};
export type SignOutSessionQuery = {
  response: SignOutSessionQuery$data;
  variables: SignOutSessionQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "personId"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "personId",
    "variableName": "personId"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "firstName",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "categories",
  "plural": true,
  "selections": [
    (v3/*: any*/),
    (v4/*: any*/)
  ],
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "startTime",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SignOutSessionQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": "PersonSignoutSession",
        "kind": "LinkedField",
        "name": "personSignoutSession",
        "plural": false,
        "selections": [
          (v2/*: any*/),
          (v5/*: any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Period",
            "kind": "LinkedField",
            "name": "openPeriods",
            "plural": true,
            "selections": [
              (v3/*: any*/),
              (v6/*: any*/),
              {
                "alias": null,
                "args": null,
                "concreteType": "Location",
                "kind": "LinkedField",
                "name": "location",
                "plural": false,
                "selections": [
                  (v4/*: any*/)
                ],
                "storageKey": null
              }
            ],
            "storageKey": null
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
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SignOutSessionQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": "PersonSignoutSession",
        "kind": "LinkedField",
        "name": "personSignoutSession",
        "plural": false,
        "selections": [
          (v2/*: any*/),
          (v5/*: any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Period",
            "kind": "LinkedField",
            "name": "openPeriods",
            "plural": true,
            "selections": [
              (v3/*: any*/),
              (v6/*: any*/),
              {
                "alias": null,
                "args": null,
                "concreteType": "Location",
                "kind": "LinkedField",
                "name": "location",
                "plural": false,
                "selections": [
                  (v4/*: any*/),
                  (v3/*: any*/)
                ],
                "storageKey": null
              }
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "cf3711cf9fb4c299c84ed1f434ea1370",
    "id": null,
    "metadata": {},
    "name": "SignOutSessionQuery",
    "operationKind": "query",
    "text": "query SignOutSessionQuery(\n  $personId: ID!\n) {\n  personSignoutSession(personId: $personId) {\n    firstName\n    categories {\n      id\n      name\n    }\n    openPeriods {\n      id\n      startTime\n      location {\n        name\n        id\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "bb872cfa73fb205626d3c05adebbd443";

export default node;
