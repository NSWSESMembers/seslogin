/**
 * @generated SignedSource<<d9fdb6ce3ba7fe7f5809acf253c2f2ae>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MembersListQuery$variables = {
  location: string;
  showBadges: boolean;
};
export type MembersListQuery$data = {
  readonly location: {
    readonly id: string;
    readonly lastSuccessfulMemberSync: number | null | undefined;
    readonly people: ReadonlyArray<{
      readonly badges?: ReadonlyArray<{
        readonly id: string;
        readonly tier: string;
      }>;
      readonly firstName: string;
      readonly id: string;
      readonly lastName: string;
      readonly memberNumber: string | null | undefined;
      readonly sesApiPersonId: string | null | undefined;
    }>;
    readonly sesApiHeadquartersId: string | null | undefined;
  };
};
export type MembersListQuery = {
  response: MembersListQuery$data;
  variables: MembersListQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "location"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "showBadges"
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
        "kind": "ScalarField",
        "name": "sesApiHeadquartersId",
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
            "args": null,
            "kind": "ScalarField",
            "name": "sesApiPersonId",
            "storageKey": null
          },
          {
            "condition": "showBadges",
            "kind": "Condition",
            "passingValue": true,
            "selections": [
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
                    "name": "tier",
                    "storageKey": null
                  }
                ],
                "storageKey": null
              }
            ]
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
    "name": "MembersListQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "MembersListQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "e613cf0fc570be846097a55fb1388e64",
    "id": null,
    "metadata": {},
    "name": "MembersListQuery",
    "operationKind": "query",
    "text": "query MembersListQuery(\n  $location: ID!\n  $showBadges: Boolean!\n) {\n  location(id: $location) {\n    id\n    sesApiHeadquartersId\n    lastSuccessfulMemberSync\n    people {\n      id\n      firstName\n      lastName\n      memberNumber\n      sesApiPersonId\n      badges(locationId: $location) @include(if: $showBadges) {\n        id\n        tier\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "1e9d237d1a211ee1bd2684738b41a388";

export default node;
