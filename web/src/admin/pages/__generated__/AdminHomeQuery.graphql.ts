/**
 * @generated SignedSource<<c7489985b12fbfde0707e78f719ed658>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type AdminHomeQuery$variables = {
  location: string;
  now: number;
};
export type AdminHomeQuery$data = {
  readonly location: {
    readonly dashboardSummary: {
      readonly activeMembers24H: number;
      readonly activeMembers24HNonVirtual: number;
      readonly activeMembers24HVirtual: number;
      readonly activeMembers30D: number;
      readonly avgCompletedDuration7D: number;
      readonly checkIns24H: number;
      readonly checkIns7D: number;
      readonly checkIns7DNonVirtual: number;
      readonly checkIns7DVirtual: number;
      readonly dailyPeriods7D: ReadonlyArray<{
        readonly dayStart: number;
        readonly periodCount: number;
        readonly periodCountVirtual: number;
        readonly totalTime: number;
        readonly totalTimeVirtual: number;
      }>;
      readonly lastSuccessfulMemberSync: number | null | undefined;
      readonly onlineKiosks: number;
      readonly recentlyActiveKiosks: number;
      readonly topCategories7D: ReadonlyArray<{
        readonly categoryId: string | null | undefined;
        readonly categoryName: string;
        readonly isVirtual: boolean;
        readonly periodCount: number;
        readonly totalTime: number;
      }>;
      readonly totalKiosks: number;
      readonly totalMembers: number;
      readonly totalTime7D: number;
      readonly totalTime7DNonVirtual: number;
      readonly totalTime7DVirtual: number;
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
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "location"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "now"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "periodCount",
  "storageKey": null
},
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "totalTime",
  "storageKey": null
},
v3 = [
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
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
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
            "name": "activeMembers24HVirtual",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "activeMembers24HNonVirtual",
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
            "name": "checkIns7DVirtual",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "checkIns7DNonVirtual",
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
            "name": "totalTime7DVirtual",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "totalTime7DNonVirtual",
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
              (v1/*: any*/),
              (v2/*: any*/),
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "periodCountVirtual",
                "storageKey": null
              },
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "totalTimeVirtual",
                "storageKey": null
              }
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
              (v1/*: any*/),
              (v2/*: any*/),
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "isVirtual",
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
    "name": "AdminHomeQuery",
    "selections": (v3/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "AdminHomeQuery",
    "selections": (v3/*: any*/)
  },
  "params": {
    "cacheID": "b22a75faa719be36fd8e9d23834b452e",
    "id": null,
    "metadata": {},
    "name": "AdminHomeQuery",
    "operationKind": "query",
    "text": "query AdminHomeQuery(\n  $location: ID!\n  $now: Int!\n) {\n  location(id: $location) {\n    id\n    name\n    dashboardSummary(asOf: $now) {\n      totalMembers\n      activeMembers24H\n      activeMembers24HVirtual\n      activeMembers24HNonVirtual\n      activeMembers30D\n      checkIns24H\n      checkIns7D\n      checkIns7DVirtual\n      checkIns7DNonVirtual\n      totalTime7D\n      totalTime7DVirtual\n      totalTime7DNonVirtual\n      avgCompletedDuration7D\n      totalKiosks\n      onlineKiosks\n      recentlyActiveKiosks\n      lastSuccessfulMemberSync\n      dailyPeriods7D {\n        dayStart\n        periodCount\n        totalTime\n        periodCountVirtual\n        totalTimeVirtual\n      }\n      topCategories7D {\n        categoryId\n        categoryName\n        periodCount\n        totalTime\n        isVirtual\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "76e10e4c7e47767d242ad3e0b7001afa";

export default node;
