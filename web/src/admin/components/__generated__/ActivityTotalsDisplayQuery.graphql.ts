/**
 * @generated SignedSource<<fbc51f38c5b69c351805f28869e6f382>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityTotalsDisplayQuery$variables = {
  categories?: ReadonlyArray<string> | null | undefined;
  endTime: number;
  location: string;
  startTime: number;
};
export type ActivityTotalsDisplayQuery$data = {
  readonly location: {
    readonly id: string;
    readonly periodSummaryByCategory: ReadonlyArray<{
      readonly category: {
        readonly id: string;
        readonly isVirtual: boolean;
        readonly name: string;
      };
      readonly totalTime: number;
    }>;
    readonly periodSummaryByMember: ReadonlyArray<{
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly isMemberOf: boolean;
        readonly lastName: string;
        readonly location: {
          readonly id: string;
          readonly name: string;
        };
      };
      readonly totalTime: number;
      readonly totalTimeVirtual: number | null | undefined;
    }>;
  };
};
export type ActivityTotalsDisplayQuery = {
  response: ActivityTotalsDisplayQuery$data;
  variables: ActivityTotalsDisplayQuery$variables;
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
v5 = {
  "kind": "Variable",
  "name": "endTime",
  "variableName": "endTime"
},
v6 = {
  "kind": "Variable",
  "name": "startTime",
  "variableName": "startTime"
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "totalTime",
  "storageKey": null
},
v9 = [
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
      (v4/*:: as any*/),
      {
        "alias": null,
        "args": [
          {
            "kind": "Variable",
            "name": "categories",
            "variableName": "categories"
          },
          (v5/*:: as any*/),
          (v6/*:: as any*/)
        ],
        "concreteType": "MemberPeriodSummary",
        "kind": "LinkedField",
        "name": "periodSummaryByMember",
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
              (v4/*:: as any*/),
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
                "concreteType": "Location",
                "kind": "LinkedField",
                "name": "location",
                "plural": false,
                "selections": [
                  (v4/*:: as any*/),
                  (v7/*:: as any*/)
                ],
                "storageKey": null
              },
              {
                "alias": null,
                "args": [
                  {
                    "kind": "Variable",
                    "name": "locationId",
                    "variableName": "location"
                  }
                ],
                "kind": "ScalarField",
                "name": "isMemberOf",
                "storageKey": null
              }
            ],
            "storageKey": null
          },
          (v8/*:: as any*/),
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
        "args": [
          (v5/*:: as any*/),
          (v6/*:: as any*/)
        ],
        "concreteType": "CategoryPeriodSummary",
        "kind": "LinkedField",
        "name": "periodSummaryByCategory",
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
              (v4/*:: as any*/),
              (v7/*:: as any*/),
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "isVirtual",
                "storageKey": null
              }
            ],
            "storageKey": null
          },
          (v8/*:: as any*/)
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
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "ActivityTotalsDisplayQuery",
    "selections": (v9/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*:: as any*/),
      (v3/*:: as any*/),
      (v1/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "ActivityTotalsDisplayQuery",
    "selections": (v9/*:: as any*/)
  },
  "params": {
    "cacheID": "b3d24758636f5b3e605cc7cca2abbf15",
    "id": null,
    "metadata": {},
    "name": "ActivityTotalsDisplayQuery",
    "operationKind": "query",
    "text": "query ActivityTotalsDisplayQuery(\n  $location: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $categories: [ID!]\n) {\n  location(id: $location) {\n    id\n    periodSummaryByMember(startTime: $startTime, endTime: $endTime, categories: $categories) {\n      person {\n        id\n        firstName\n        lastName\n        location {\n          id\n          name\n        }\n        isMemberOf(locationId: $location)\n      }\n      totalTime\n      totalTimeVirtual\n    }\n    periodSummaryByCategory(startTime: $startTime, endTime: $endTime) {\n      category {\n        id\n        name\n        isVirtual\n      }\n      totalTime\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "72f787da300338d7012c6b493e4f28a1";

export default node;
