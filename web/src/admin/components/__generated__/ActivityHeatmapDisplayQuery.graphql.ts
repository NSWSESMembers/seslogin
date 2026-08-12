/**
 * @generated SignedSource<<ba8445dbdc8d35b066c0ba3469a7733c>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityHeatmapDisplayQuery$variables = {
  category?: string | null | undefined;
  endTime: number;
  location: string;
  startTime: number;
};
export type ActivityHeatmapDisplayQuery$data = {
  readonly location: {
    readonly id: string;
    readonly people: ReadonlyArray<{
      readonly firstName: string;
      readonly id: string;
      readonly lastName: string;
    }>;
    readonly periodSummaryByDayByMember: ReadonlyArray<{
      readonly date: string;
      readonly members: ReadonlyArray<{
        readonly periodCount: number;
        readonly person: {
          readonly id: string;
        };
        readonly totalTime: number;
      }>;
    }>;
  };
};
export type ActivityHeatmapDisplayQuery = {
  response: ActivityHeatmapDisplayQuery$data;
  variables: ActivityHeatmapDisplayQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "category"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "endTime"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "location"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v5 = [
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
      (v4/*: any*/),
      {
        "alias": null,
        "args": null,
        "concreteType": "Person",
        "kind": "LinkedField",
        "name": "people",
        "plural": true,
        "selections": [
          (v4/*: any*/),
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
        "args": [
          {
            "kind": "Variable",
            "name": "category",
            "variableName": "category"
          },
          {
            "kind": "Variable",
            "name": "endTime",
            "variableName": "endTime"
          },
          {
            "kind": "Variable",
            "name": "startTime",
            "variableName": "startTime"
          }
        ],
        "concreteType": "DayMemberPeriodSummary",
        "kind": "LinkedField",
        "name": "periodSummaryByDayByMember",
        "plural": true,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "date",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "MemberDayPeriodSummary",
            "kind": "LinkedField",
            "name": "members",
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
                  (v4/*: any*/)
                ],
                "storageKey": null
              },
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "totalTime",
                "storageKey": null
              },
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "periodCount",
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
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityHeatmapDisplayQuery",
    "selections": (v5/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*: any*/),
      (v3/*: any*/),
      (v1/*: any*/),
      (v0/*: any*/)
    ],
    "kind": "Operation",
    "name": "ActivityHeatmapDisplayQuery",
    "selections": (v5/*: any*/)
  },
  "params": {
    "cacheID": "57cc96e8248f4042990358b676829df3",
    "id": null,
    "metadata": {},
    "name": "ActivityHeatmapDisplayQuery",
    "operationKind": "query",
    "text": "query ActivityHeatmapDisplayQuery(\n  $location: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $category: ID\n) {\n  location(id: $location) {\n    id\n    people {\n      id\n      firstName\n      lastName\n    }\n    periodSummaryByDayByMember(startTime: $startTime, endTime: $endTime, category: $category) {\n      date\n      members {\n        person {\n          id\n        }\n        totalTime\n        periodCount\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "8cc6ec0cf62c51079478f844a72502d4";

export default node;
