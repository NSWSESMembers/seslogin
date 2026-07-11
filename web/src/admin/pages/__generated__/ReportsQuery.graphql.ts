/**
 * @generated SignedSource<<38ad635810139fbcfe2c3e13802846b8>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ReportsQuery$variables = {
  after?: string | null | undefined;
  endTime: number;
  first: number;
  location: string;
  startTime: number;
};
export type ReportsQuery$data = {
  readonly location: {
    readonly id: string;
    readonly periods: {
      readonly edges: ReadonlyArray<{
        readonly node: {
          readonly category: {
            readonly id: string;
            readonly name: string;
          } | null | undefined;
          readonly comment: string | null | undefined;
          readonly endTime: number | null | undefined;
          readonly guestName: string | null | undefined;
          readonly id: string;
          readonly person: {
            readonly firstName: string;
            readonly id: string;
            readonly lastName: string;
            readonly memberNumber: string | null | undefined;
          } | null | undefined;
          readonly personId: string | null | undefined;
          readonly signedInSession: {
            readonly name: string;
          } | null | undefined;
          readonly signedOutSession: {
            readonly name: string;
          } | null | undefined;
          readonly startTime: number;
        };
      }>;
      readonly pageInfo: {
        readonly endCursor: string | null | undefined;
        readonly hasNextPage: boolean;
      };
    };
  };
};
export type ReportsQuery = {
  response: ReportsQuery$data;
  variables: ReportsQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "after"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "endTime"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "first"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "location"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
},
v5 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "location"
  }
],
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v7 = [
  {
    "kind": "Variable",
    "name": "after",
    "variableName": "after"
  },
  {
    "kind": "Variable",
    "name": "endTime",
    "variableName": "endTime"
  },
  {
    "kind": "Variable",
    "name": "first",
    "variableName": "first"
  },
  {
    "kind": "Variable",
    "name": "startTime",
    "variableName": "startTime"
  }
],
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "personId",
  "storageKey": null
},
v9 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "guestName",
  "storageKey": null
},
v10 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "comment",
  "storageKey": null
},
v11 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "startTime",
  "storageKey": null
},
v12 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "endTime",
  "storageKey": null
},
v13 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v14 = [
  (v13/*: any*/)
],
v15 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "category",
  "plural": false,
  "selections": [
    (v6/*: any*/),
    (v13/*: any*/)
  ],
  "storageKey": null
},
v16 = {
  "alias": null,
  "args": null,
  "concreteType": "Person",
  "kind": "LinkedField",
  "name": "person",
  "plural": false,
  "selections": [
    (v6/*: any*/),
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "memberNumber",
      "storageKey": null
    },
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
v17 = {
  "alias": null,
  "args": null,
  "concreteType": "PageInfo",
  "kind": "LinkedField",
  "name": "pageInfo",
  "plural": false,
  "selections": [
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "hasNextPage",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "endCursor",
      "storageKey": null
    }
  ],
  "storageKey": null
},
v18 = [
  (v13/*: any*/),
  (v6/*: any*/)
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/),
      (v4/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ReportsQuery",
    "selections": [
      {
        "alias": null,
        "args": (v5/*: any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v6/*: any*/),
          {
            "alias": null,
            "args": (v7/*: any*/),
            "concreteType": "PeriodConnection",
            "kind": "LinkedField",
            "name": "periods",
            "plural": false,
            "selections": [
              {
                "alias": null,
                "args": null,
                "concreteType": "PeriodEdge",
                "kind": "LinkedField",
                "name": "edges",
                "plural": true,
                "selections": [
                  {
                    "alias": null,
                    "args": null,
                    "concreteType": "Period",
                    "kind": "LinkedField",
                    "name": "node",
                    "plural": false,
                    "selections": [
                      (v6/*: any*/),
                      (v8/*: any*/),
                      (v9/*: any*/),
                      (v10/*: any*/),
                      (v11/*: any*/),
                      (v12/*: any*/),
                      {
                        "alias": null,
                        "args": null,
                        "concreteType": "Session",
                        "kind": "LinkedField",
                        "name": "signedInSession",
                        "plural": false,
                        "selections": (v14/*: any*/),
                        "storageKey": null
                      },
                      {
                        "alias": null,
                        "args": null,
                        "concreteType": "Session",
                        "kind": "LinkedField",
                        "name": "signedOutSession",
                        "plural": false,
                        "selections": (v14/*: any*/),
                        "storageKey": null
                      },
                      (v15/*: any*/),
                      (v16/*: any*/)
                    ],
                    "storageKey": null
                  }
                ],
                "storageKey": null
              },
              (v17/*: any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v3/*: any*/),
      (v2/*: any*/),
      (v0/*: any*/),
      (v4/*: any*/),
      (v1/*: any*/)
    ],
    "kind": "Operation",
    "name": "ReportsQuery",
    "selections": [
      {
        "alias": null,
        "args": (v5/*: any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v6/*: any*/),
          {
            "alias": null,
            "args": (v7/*: any*/),
            "concreteType": "PeriodConnection",
            "kind": "LinkedField",
            "name": "periods",
            "plural": false,
            "selections": [
              {
                "alias": null,
                "args": null,
                "concreteType": "PeriodEdge",
                "kind": "LinkedField",
                "name": "edges",
                "plural": true,
                "selections": [
                  {
                    "alias": null,
                    "args": null,
                    "concreteType": "Period",
                    "kind": "LinkedField",
                    "name": "node",
                    "plural": false,
                    "selections": [
                      (v6/*: any*/),
                      (v8/*: any*/),
                      (v9/*: any*/),
                      (v10/*: any*/),
                      (v11/*: any*/),
                      (v12/*: any*/),
                      {
                        "alias": null,
                        "args": null,
                        "concreteType": "Session",
                        "kind": "LinkedField",
                        "name": "signedInSession",
                        "plural": false,
                        "selections": (v18/*: any*/),
                        "storageKey": null
                      },
                      {
                        "alias": null,
                        "args": null,
                        "concreteType": "Session",
                        "kind": "LinkedField",
                        "name": "signedOutSession",
                        "plural": false,
                        "selections": (v18/*: any*/),
                        "storageKey": null
                      },
                      (v15/*: any*/),
                      (v16/*: any*/)
                    ],
                    "storageKey": null
                  }
                ],
                "storageKey": null
              },
              (v17/*: any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "b62e61d7035e61ba1666ea0b3a069927",
    "id": null,
    "metadata": {},
    "name": "ReportsQuery",
    "operationKind": "query",
    "text": "query ReportsQuery(\n  $location: ID!\n  $first: Int!\n  $after: String\n  $startTime: Int!\n  $endTime: Int!\n) {\n  location(id: $location) {\n    id\n    periods(first: $first, after: $after, startTime: $startTime, endTime: $endTime) {\n      edges {\n        node {\n          id\n          personId\n          guestName\n          comment\n          startTime\n          endTime\n          signedInSession {\n            name\n            id\n          }\n          signedOutSession {\n            name\n            id\n          }\n          category {\n            id\n            name\n          }\n          person {\n            id\n            memberNumber\n            firstName\n            lastName\n          }\n        }\n      }\n      pageInfo {\n        hasNextPage\n        endCursor\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "76b34a0fd35ccc0c35d7095c02d8cf2c";

export default node;
