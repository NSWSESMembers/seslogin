/**
 * @generated SignedSource<<92c2c3db1c213fb36c9e4a21909999fa>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanTitleBarMemberBadgesQuery$variables = {
  id: string;
  locationId: string;
};
export type ScanTitleBarMemberBadgesQuery$data = {
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
    readonly id: string;
    readonly memberNumber: string | null | undefined;
  };
};
export type ScanTitleBarMemberBadgesQuery = {
  response: ScanTitleBarMemberBadgesQuery$data;
  variables: ScanTitleBarMemberBadgesQuery$variables;
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
      (v1/*:: as any*/),
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
          (v1/*:: as any*/),
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
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanTitleBarMemberBadgesQuery",
    "selections": (v2/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "ScanTitleBarMemberBadgesQuery",
    "selections": (v2/*:: as any*/)
  },
  "params": {
    "cacheID": "516b789c64cc39779357caeb381921d5",
    "id": null,
    "metadata": {},
    "name": "ScanTitleBarMemberBadgesQuery",
    "operationKind": "query",
    "text": "query ScanTitleBarMemberBadgesQuery(\n  $id: ID!\n  $locationId: ID!\n) {\n  person(id: $id) {\n    id\n    memberNumber\n    badgeProgress(locationId: $locationId) {\n      id\n      badgeId\n      name\n      description\n      tier\n      source\n      earned\n      awardedAt\n      current\n      target\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "740d86590cb80528067cb5567ab8c822";

export default node;
