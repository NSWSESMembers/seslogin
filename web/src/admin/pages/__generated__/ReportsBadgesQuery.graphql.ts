/**
 * @generated SignedSource<<0f014686995e7badc0b3345e9851a7ba>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ReportsBadgesQuery$variables = {
  location: string;
};
export type ReportsBadgesQuery$data = {
  readonly location: {
    readonly id: string;
    readonly people: ReadonlyArray<{
      readonly badges: ReadonlyArray<{
        readonly awardedAt: number;
        readonly id: string;
        readonly name: string;
      }>;
      readonly firstName: string;
      readonly id: string;
      readonly lastName: string;
      readonly memberNumber: string | null | undefined;
    }>;
  };
};
export type ReportsBadgesQuery = {
  response: ReportsBadgesQuery$data;
  variables: ReportsBadgesQuery$variables;
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
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "memberNumber",
            "storageKey": null
          },
          {
            "alias": null,
            "args": [
              {
                "kind": "Variable",
                "name": "locationId",
                "variableName": "location"
              }
            ],
            "concreteType": "PersonBadge",
            "kind": "LinkedField",
            "name": "badges",
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
                "name": "awardedAt",
                "storageKey": null
              }
            ],
            "storageKey": null
          }
        ],
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
    "name": "ReportsBadgesQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ReportsBadgesQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "cad2113a79ea670c13f5d988238d12eb",
    "id": null,
    "metadata": {},
    "name": "ReportsBadgesQuery",
    "operationKind": "query",
    "text": "query ReportsBadgesQuery(\n  $location: ID!\n) {\n  location(id: $location) {\n    id\n    people {\n      id\n      firstName\n      lastName\n      memberNumber\n      badges(locationId: $location) {\n        id\n        name\n        awardedAt\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "d4825861613d55fdfbc06057f4103527";

export default node;
