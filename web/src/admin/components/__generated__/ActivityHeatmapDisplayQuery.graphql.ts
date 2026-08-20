/**
 * @generated SignedSource<<d9d38c2f0bee251b69bbc47e3d8c255f>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityHeatmapDisplayQuery$variables = {
  categories?: ReadonlyArray<string> | null | undefined;
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
  "name": "categories"
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
            "name": "categories",
            "variableName": "categories"
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
    "metadata": {
      "throwOnFieldError": true
    },
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
    "cacheID": "c8884b87e62d253cfc606dab5e6c41d9",
    "id": null,
    "metadata": {},
    "name": "ActivityHeatmapDisplayQuery",
    "operationKind": "query",
    "text": "query ActivityHeatmapDisplayQuery(\n  $location: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $categories: [ID!]\n) {\n  location(id: $location) {\n    id\n    people {\n      id\n      firstName\n      lastName\n    }\n    periodSummaryByDayByMember(startTime: $startTime, endTime: $endTime, categories: $categories) {\n      date\n      members {\n        person {\n          id\n        }\n        totalTime\n        periodCount\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "47d6a519d9b266bf8dde3872757bab06";

export default node;
