/**
 * @generated SignedSource<<7395399c8cf12677563e486767ee1150>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MembersBadgesQuery$variables = {
  id: string;
  locationId: string;
};
export type MembersBadgesQuery$data = {
  readonly person: {
    readonly badgeProgress: ReadonlyArray<{
      readonly awardedAt: number | null | undefined;
      readonly badgeId: string;
      readonly current: number | null | undefined;
      readonly description: string;
      readonly earned: boolean;
      readonly id: string;
      readonly name: string;
      readonly source: string;
      readonly target: number | null | undefined;
      readonly tier: string;
    }>;
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
    readonly memberNumber: string | null | undefined;
  };
};
export type MembersBadgesQuery = {
  response: MembersBadgesQuery$data;
  variables: MembersBadgesQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "locationId"
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
        "variableName": "id"
      }
    ],
    "concreteType": "Person",
    "kind": "LinkedField",
    "name": "person",
    "plural": false,
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
            "variableName": "locationId"
          }
        ],
        "concreteType": "PersonBadgeProgress",
        "kind": "LinkedField",
        "name": "badgeProgress",
        "plural": true,
        "selections": [
          (v1/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "badgeId",
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
            "name": "description",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "tier",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "source",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "earned",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "awardedAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "current",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "target",
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
    "name": "MembersBadgesQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "MembersBadgesQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "b169128f724bd9a7841a7321bb9ecb04",
    "id": null,
    "metadata": {},
    "name": "MembersBadgesQuery",
    "operationKind": "query",
    "text": "query MembersBadgesQuery(\n  $id: ID!\n  $locationId: ID!\n) {\n  person(id: $id) {\n    id\n    firstName\n    lastName\n    memberNumber\n    badgeProgress(locationId: $locationId) {\n      id\n      badgeId\n      name\n      description\n      tier\n      source\n      earned\n      awardedAt\n      current\n      target\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "18aec89d680a68b1313c02733aa56c3e";

export default node;
