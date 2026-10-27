/**
 * @generated SignedSource<<c94067551200dcc86d71495b1b9b767b>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanTitleBarLeaderboardQuery$variables = {
  limit?: number | null | undefined;
  locationId: string;
};
export type ScanTitleBarLeaderboardQuery$data = {
  readonly location: {
    readonly badgeLeaderboard: ReadonlyArray<{
      readonly badgeCount: number;
      readonly latestBadgeAwardAt: number;
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
      };
      readonly recentBadgeCount7D: number;
      readonly tierCounts: ReadonlyArray<{
        readonly count: number;
        readonly tier: string;
      }>;
    }>;
    readonly id: string;
  };
};
export type ScanTitleBarLeaderboardQuery = {
  response: ScanTitleBarLeaderboardQuery$data;
  variables: ScanTitleBarLeaderboardQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "limit"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "locationId"
},
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v3 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "locationId"
      }
    ],
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "location",
    "plural": false,
    "selections": [
      (v2/*:: as any*/),
      {
        "alias": null,
        "args": [
          {
            "kind": "Variable",
            "name": "limit",
            "variableName": "limit"
          }
        ],
        "concreteType": "LocationBadgeLeaderboardEntry",
        "kind": "LinkedField",
        "name": "badgeLeaderboard",
        "plural": true,
        "selections": [
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v2/*:: as any*/),
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
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "badgeCount",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "recentBadgeCount7D",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "latestBadgeAwardAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "BadgeTierCount",
            "kind": "LinkedField",
            "name": "tierCounts",
            "plural": true,
            "selections": [
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
                "name": "count",
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
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanTitleBarLeaderboardQuery",
    "selections": (v3/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "ScanTitleBarLeaderboardQuery",
    "selections": (v3/*:: as any*/)
  },
  "params": {
    "cacheID": "068dfa2fb0d4c6bdd80994a18efa8ace",
    "id": null,
    "metadata": {},
    "name": "ScanTitleBarLeaderboardQuery",
    "operationKind": "query",
    "text": "query ScanTitleBarLeaderboardQuery(\n  $locationId: ID!\n  $limit: Int\n) {\n  location(id: $locationId) {\n    id\n    badgeLeaderboard(limit: $limit) {\n      person {\n        id\n        firstName\n        lastName\n      }\n      badgeCount\n      recentBadgeCount7D\n      latestBadgeAwardAt\n      tierCounts {\n        tier\n        count\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "481b64a77a47ab075b29ece58ba16bcf";

export default node;
