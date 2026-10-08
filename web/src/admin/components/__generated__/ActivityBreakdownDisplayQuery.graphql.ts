/**
 * @generated SignedSource<<90c0ae0b2e00075f1d52321fe7862684>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityBreakdownDisplayQuery$variables = {
  endTime: number;
  location: string;
  startTime: number;
};
export type ActivityBreakdownDisplayQuery$data = {
  readonly location: {
    readonly id: string;
    readonly periodSummaryByCategoryByMember: ReadonlyArray<{
      readonly category: {
        readonly id: string;
        readonly isVirtual: boolean;
        readonly name: string;
      };
      readonly members: ReadonlyArray<{
        readonly person: {
          readonly firstName: string;
          readonly id: string;
          readonly lastName: string;
          readonly location: {
            readonly id: string;
            readonly name: string;
          };
        };
        readonly totalTime: number;
      }>;
      readonly totalTime: number;
    }>;
    readonly periodSummaryByMemberByCategory: ReadonlyArray<{
      readonly categories: ReadonlyArray<{
        readonly category: {
          readonly id: string;
          readonly isVirtual: boolean;
          readonly name: string;
        };
        readonly totalTime: number;
      }>;
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
        readonly location: {
          readonly id: string;
          readonly name: string;
        };
      };
      readonly totalTime: number;
    }>;
  };
};
export type ActivityBreakdownDisplayQuery = {
  response: ActivityBreakdownDisplayQuery$data;
  variables: ActivityBreakdownDisplayQuery$variables;
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
v4 = [
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
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "concreteType": "Person",
  "kind": "LinkedField",
  "name": "person",
  "plural": false,
  "selections": [
    (v3/*:: as any*/),
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
        (v3/*:: as any*/),
        (v5/*:: as any*/)
      ],
      "storageKey": null
    }
  ],
  "storageKey": null
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "totalTime",
  "storageKey": null
},
v8 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "category",
  "plural": false,
  "selections": [
    (v3/*:: as any*/),
    (v5/*:: as any*/),
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
      (v3/*:: as any*/),
      {
        "alias": null,
        "args": (v4/*:: as any*/),
        "concreteType": "MemberCategoryPeriodSummary",
        "kind": "LinkedField",
        "name": "periodSummaryByMemberByCategory",
        "plural": true,
        "selections": [
          (v6/*:: as any*/),
          (v7/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "CategoryPeriodSummary",
            "kind": "LinkedField",
            "name": "categories",
            "plural": true,
            "selections": [
              (v8/*:: as any*/),
              (v7/*:: as any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": (v4/*:: as any*/),
        "concreteType": "CategoryMemberPeriodSummary",
        "kind": "LinkedField",
        "name": "periodSummaryByCategoryByMember",
        "plural": true,
        "selections": [
          (v8/*:: as any*/),
          (v7/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "MemberPeriodSummary",
            "kind": "LinkedField",
            "name": "members",
            "plural": true,
            "selections": [
              (v6/*:: as any*/),
              (v7/*:: as any*/)
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
      (v1/*:: as any*/),
      (v2/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "ActivityBreakdownDisplayQuery",
    "selections": (v9/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "ActivityBreakdownDisplayQuery",
    "selections": (v9/*:: as any*/)
  },
  "params": {
    "cacheID": "d0eddfa011816ba916ad8d585eb08a63",
    "id": null,
    "metadata": {},
    "name": "ActivityBreakdownDisplayQuery",
    "operationKind": "query",
    "text": "query ActivityBreakdownDisplayQuery(\n  $location: ID!\n  $startTime: Int!\n  $endTime: Int!\n) {\n  location(id: $location) {\n    id\n    periodSummaryByMemberByCategory(startTime: $startTime, endTime: $endTime) {\n      person {\n        id\n        firstName\n        lastName\n        location {\n          id\n          name\n        }\n      }\n      totalTime\n      categories {\n        category {\n          id\n          name\n          isVirtual\n        }\n        totalTime\n      }\n    }\n    periodSummaryByCategoryByMember(startTime: $startTime, endTime: $endTime) {\n      category {\n        id\n        name\n        isVirtual\n      }\n      totalTime\n      members {\n        person {\n          id\n          firstName\n          lastName\n          location {\n            id\n            name\n          }\n        }\n        totalTime\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "318937614fc676bff354be466bc2e57e";

export default node;
