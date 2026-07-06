/**
 * @generated SignedSource<<16fce9fd5fb232fac7ec5daaacb4c1e9>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityDailyBreakdownDisplayQuery$variables = {
  endTime: number;
  location: string;
  startTime: number;
};
export type ActivityDailyBreakdownDisplayQuery$data = {
  readonly location: {
    readonly id: string;
    readonly periodSummaryByDayByCategoryByMember: ReadonlyArray<{
      readonly categories: ReadonlyArray<{
        readonly category: {
          readonly id: string;
          readonly name: string;
        };
        readonly members: ReadonlyArray<{
          readonly person: {
            readonly firstName: string;
            readonly id: string;
            readonly lastName: string;
          };
          readonly totalTime: number;
        }>;
        readonly totalTime: number;
      }>;
      readonly date: string;
      readonly totalTime: number;
    }>;
  };
};
export type ActivityDailyBreakdownDisplayQuery = {
  response: ActivityDailyBreakdownDisplayQuery$data;
  variables: ActivityDailyBreakdownDisplayQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "endTime"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "location"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
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
  "name": "totalTime",
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
      (v3/*: any*/),
      {
        "alias": null,
        "args": [
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
        "concreteType": "DayCategoryPeriodSummary",
        "kind": "LinkedField",
        "name": "periodSummaryByDayByCategoryByMember",
        "plural": true,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "date",
            "storageKey": null
          },
          (v4/*: any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "CategoryMemberPeriodSummary",
            "kind": "LinkedField",
            "name": "categories",
            "plural": true,
            "selections": [
              {
                "alias": null,
                "args": null,
                "concreteType": "Category",
                "kind": "LinkedField",
                "name": "category",
                "plural": false,
                "selections": [
                  (v3/*: any*/),
                  {
                    "alias": null,
                    "args": null,
                    "kind": "ScalarField",
                    "name": "name",
                    "storageKey": null
                  }
                ],
                "storageKey": null
              },
              (v4/*: any*/),
              {
                "alias": null,
                "args": null,
                "concreteType": "MemberPeriodSummary",
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
                      }
                    ],
                    "storageKey": null
                  },
                  (v4/*: any*/)
                ],
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
      (v2/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityDailyBreakdownDisplayQuery",
    "selections": (v5/*: any*/),
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
    "name": "ActivityDailyBreakdownDisplayQuery",
    "selections": (v5/*: any*/)
  },
  "params": {
    "cacheID": "e10f7414998bfbe6c51f29d1d74c65f7",
    "id": null,
    "metadata": {},
    "name": "ActivityDailyBreakdownDisplayQuery",
    "operationKind": "query",
    "text": "query ActivityDailyBreakdownDisplayQuery(\n  $location: ID!\n  $startTime: Int!\n  $endTime: Int!\n) {\n  location(id: $location) {\n    id\n    periodSummaryByDayByCategoryByMember(startTime: $startTime, endTime: $endTime) {\n      date\n      totalTime\n      categories {\n        category {\n          id\n          name\n        }\n        totalTime\n        members {\n          person {\n            id\n            firstName\n            lastName\n          }\n          totalTime\n        }\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "ed0f3b315ac1c40a7e4a5e2d581072bd";

export default node;
