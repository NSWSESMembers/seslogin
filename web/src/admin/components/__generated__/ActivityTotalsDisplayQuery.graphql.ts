/**
 * @generated SignedSource<<c31c4456f3a45dc342baa1ac95d14ae4>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityTotalsDisplayQuery$variables = {
  category?: string | null | undefined;
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
        readonly name: string;
      };
      readonly totalTime: number;
    }>;
    readonly periodSummaryByMember: ReadonlyArray<{
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
      };
      readonly totalTime: number;
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
  "name": "totalTime",
  "storageKey": null
},
v8 = [
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
        "args": [
          {
            "kind": "Variable",
            "name": "category",
            "variableName": "category"
          },
          (v5/*: any*/),
          (v6/*: any*/)
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
          (v7/*: any*/)
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": [
          (v5/*: any*/),
          (v6/*: any*/)
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
              (v4/*: any*/),
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
          (v7/*: any*/)
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
    "name": "ActivityTotalsDisplayQuery",
    "selections": (v8/*: any*/),
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
    "name": "ActivityTotalsDisplayQuery",
    "selections": (v8/*: any*/)
  },
  "params": {
    "cacheID": "b35ad60995354dc632fdbf7e175dd548",
    "id": null,
    "metadata": {},
    "name": "ActivityTotalsDisplayQuery",
    "operationKind": "query",
    "text": "query ActivityTotalsDisplayQuery(\n  $location: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $category: ID\n) {\n  location(id: $location) {\n    id\n    periodSummaryByMember(startTime: $startTime, endTime: $endTime, category: $category) {\n      person {\n        id\n        firstName\n        lastName\n      }\n      totalTime\n    }\n    periodSummaryByCategory(startTime: $startTime, endTime: $endTime) {\n      category {\n        id\n        name\n      }\n      totalTime\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "6fa73a68c7ff2230ddb758171cf20565";

export default node;
