/**
 * @generated SignedSource<<547a8036595e4348fef36ea27f73b39a>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanMemberBadgesModalQuery$variables = {
  id: string;
  locationId: string;
};
export type ScanMemberBadgesModalQuery$data = {
  readonly person: {
    readonly badges: ReadonlyArray<{
      readonly awardedAt: number;
      readonly description: string;
      readonly id: string;
      readonly name: string;
      readonly tier: string;
    }>;
    readonly id: string;
  };
};
export type ScanMemberBadgesModalQuery = {
  response: ScanMemberBadgesModalQuery$data;
  variables: ScanMemberBadgesModalQuery$variables;
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
        "args": [
          {
            "kind": "Variable",
            "name": "locationId",
            "variableName": "locationId"
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
    "name": "ScanMemberBadgesModalQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ScanMemberBadgesModalQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "6c94b4578838e42d34e87c75035eab21",
    "id": null,
    "metadata": {},
    "name": "ScanMemberBadgesModalQuery",
    "operationKind": "query",
    "text": "query ScanMemberBadgesModalQuery(\n  $id: ID!\n  $locationId: ID!\n) {\n  person(id: $id) {\n    id\n    badges(locationId: $locationId) {\n      id\n      name\n      description\n      tier\n      awardedAt\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "b45c238524c42535ed24a7f7dc77c467";

export default node;
