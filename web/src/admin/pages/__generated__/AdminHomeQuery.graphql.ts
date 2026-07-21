/**
 * @generated SignedSource<<8d4c37232a2c793d206132763f1a6589>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type AdminHomeQuery$variables = {
  includeBadgeLeaderboard: boolean;
  location: string;
  now: number;
};
export type AdminHomeQuery$data = {
  readonly location: {
    readonly badgeLeaderboard?: ReadonlyArray<{
      readonly badgeCount: number;
      readonly latestBadgeAwardAt: number;
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
        readonly memberNumber: string | null | undefined;
      };
      readonly recentBadgeCount7D: number;
    }>;
    readonly dashboardSummary: {
      readonly activeMembers24H: number;
      readonly activeMembers30D: number;
      readonly avgCompletedDuration7D: number;
      readonly checkIns24H: number;
      readonly checkIns7D: number;
      readonly dailyPeriods7D: ReadonlyArray<{
        readonly dayStart: number;
        readonly periodCount: number;
        readonly totalTime: number;
      }>;
      readonly lastSuccessfulMemberSync: number | null | undefined;
      readonly onlineKiosks: number;
      readonly recentlyActiveKiosks: number;
      readonly topCategories7D: ReadonlyArray<{
        readonly categoryId: string | null | undefined;
        readonly categoryName: string;
        readonly periodCount: number;
        readonly totalTime: number;
      }>;
      readonly totalKiosks: number;
      readonly totalMembers: number;
      readonly totalTime7D: number;
    };
    readonly id: string;
    readonly name: string;
  };
};
export type AdminHomeQuery = {
  response: AdminHomeQuery$data;
  variables: AdminHomeQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "includeBadgeLeaderboard"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "location"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "now"
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
  "name": "periodCount",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "totalTime",
  "storageKey": null
},
v6 = [
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
      (v3/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "name",
        "storageKey": null
      },
      {
        "alias": null,
        "args": [
          {
            "kind": "Variable",
            "name": "asOf",
            "variableName": "now"
          }
        ],
        "concreteType": "LocationDashboardSummary",
        "kind": "LinkedField",
        "name": "dashboardSummary",
        "plural": false,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "totalMembers",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "activeMembers24H",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "activeMembers30D",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "checkIns24H",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "checkIns7D",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "totalTime7D",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "avgCompletedDuration7D",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "totalKiosks",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "onlineKiosks",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "recentlyActiveKiosks",
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
            "concreteType": "DashboardDailyPeriodSummary",
            "kind": "LinkedField",
            "name": "dailyPeriods7D",
            "plural": true,
            "selections": [
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "dayStart",
                "storageKey": null
              },
              (v4/*: any*/),
              (v5/*: any*/)
            ],
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "DashboardCategoryPeriodSummary",
            "kind": "LinkedField",
            "name": "topCategories7D",
            "plural": true,
            "selections": [
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "categoryId",
                "storageKey": null
              },
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "categoryName",
                "storageKey": null
              },
              (v4/*: any*/),
              (v5/*: any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "condition": "includeBadgeLeaderboard",
        "kind": "Condition",
        "passingValue": true,
        "selections": [
          {
            "alias": null,
            "args": [
              {
                "kind": "Literal",
                "name": "limit",
                "value": 8
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
                  (v3/*: any*/),
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
              }
            ],
            "storageKey": "badgeLeaderboard(limit:8)"
          }
        ]
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "AdminHomeQuery",
    "selections": (v6/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*: any*/),
      (v2/*: any*/),
      (v0/*: any*/)
    ],
    "kind": "Operation",
    "name": "AdminHomeQuery",
    "selections": (v6/*: any*/)
  },
  "params": {
    "cacheID": "d3a22bc3698beece40a08982faec9fa6",
    "id": null,
    "metadata": {},
    "name": "AdminHomeQuery",
    "operationKind": "query",
    "text": "query AdminHomeQuery(\n  $location: ID!\n  $now: Int!\n  $includeBadgeLeaderboard: Boolean!\n) {\n  location(id: $location) {\n    id\n    name\n    dashboardSummary(asOf: $now) {\n      totalMembers\n      activeMembers24H\n      activeMembers30D\n      checkIns24H\n      checkIns7D\n      totalTime7D\n      avgCompletedDuration7D\n      totalKiosks\n      onlineKiosks\n      recentlyActiveKiosks\n      lastSuccessfulMemberSync\n      dailyPeriods7D {\n        dayStart\n        periodCount\n        totalTime\n      }\n      topCategories7D {\n        categoryId\n        categoryName\n        periodCount\n        totalTime\n      }\n    }\n    badgeLeaderboard(limit: 8) @include(if: $includeBadgeLeaderboard) {\n      person {\n        id\n        firstName\n        lastName\n        memberNumber\n      }\n      badgeCount\n      recentBadgeCount7D\n      latestBadgeAwardAt\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "2071ca9f8db90a42ab12849b0a4e931e";

export default node;
