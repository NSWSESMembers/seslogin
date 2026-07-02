/**
 * @generated SignedSource<<5e6dc0a6ffecc67610da515fab856991>>
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
      readonly description: string;
      readonly earned: boolean;
      readonly id: string;
      readonly name: string;
      readonly source: string;
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
    "cacheID": "9832fae42f6e44343822f54ffc49ae62",
    "id": null,
    "metadata": {},
    "name": "MembersBadgesQuery",
    "operationKind": "query",
    "text": "query MembersBadgesQuery(\n  $id: ID!\n  $locationId: ID!\n) {\n  person(id: $id) {\n    id\n    firstName\n    lastName\n    memberNumber\n    badgeProgress(locationId: $locationId) {\n      id\n      name\n      description\n      tier\n      source\n      earned\n      awardedAt\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "daf2f21ae2caba30865bcbc9f13d98bf";

export default node;
