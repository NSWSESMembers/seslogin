/**
 * @generated SignedSource<<f7ed4a5abf4543f051432b3a083e8279>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { Result } from "relay-runtime";
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
          readonly category: Result<{
            readonly id: string;
            readonly name: string;
          } | null | undefined, unknown>;
          readonly comment: string | null | undefined;
          readonly endTime: number | null | undefined;
          readonly guestName: string | null | undefined;
          readonly id: string;
          readonly person: Result<{
            readonly firstName: string;
            readonly id: string;
            readonly lastName: string;
            readonly memberNumber: string | null | undefined;
          } | null | undefined, unknown>;
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
  (v13/*:: as any*/)
],
v15 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "category",
  "plural": false,
  "selections": [
    (v6/*:: as any*/),
    (v13/*:: as any*/)
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
    (v6/*:: as any*/),
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
  (v13/*:: as any*/),
  (v6/*:: as any*/)
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/),
      (v4/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "ReportsQuery",
    "selections": [
      {
        "alias": null,
        "args": (v5/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v6/*:: as any*/),
          {
            "alias": null,
            "args": (v7/*:: as any*/),
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
                      (v6/*:: as any*/),
                      (v8/*:: as any*/),
                      (v9/*:: as any*/),
                      (v10/*:: as any*/),
                      (v11/*:: as any*/),
                      (v12/*:: as any*/),
                      {
                        "alias": null,
                        "args": null,
                        "concreteType": "Session",
                        "kind": "LinkedField",
                        "name": "signedInSession",
                        "plural": false,
                        "selections": (v14/*:: as any*/),
                        "storageKey": null
                      },
                      {
                        "alias": null,
                        "args": null,
                        "concreteType": "Session",
                        "kind": "LinkedField",
                        "name": "signedOutSession",
                        "plural": false,
                        "selections": (v14/*:: as any*/),
                        "storageKey": null
                      },
                      {
                        "kind": "CatchField",
                        "field": (v15/*:: as any*/),
                        "to": "RESULT"
                      },
                      {
                        "kind": "CatchField",
                        "field": (v16/*:: as any*/),
                        "to": "RESULT"
                      }
                    ],
                    "storageKey": null
                  }
                ],
                "storageKey": null
              },
              (v17/*:: as any*/)
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
      (v3/*:: as any*/),
      (v2/*:: as any*/),
      (v0/*:: as any*/),
      (v4/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "ReportsQuery",
    "selections": [
      {
        "alias": null,
        "args": (v5/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v6/*:: as any*/),
          {
            "alias": null,
            "args": (v7/*:: as any*/),
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
                      (v6/*:: as any*/),
                      (v8/*:: as any*/),
                      (v9/*:: as any*/),
                      (v10/*:: as any*/),
                      (v11/*:: as any*/),
                      (v12/*:: as any*/),
                      {
                        "alias": null,
                        "args": null,
                        "concreteType": "Session",
                        "kind": "LinkedField",
                        "name": "signedInSession",
                        "plural": false,
                        "selections": (v18/*:: as any*/),
                        "storageKey": null
                      },
                      {
                        "alias": null,
                        "args": null,
                        "concreteType": "Session",
                        "kind": "LinkedField",
                        "name": "signedOutSession",
                        "plural": false,
                        "selections": (v18/*:: as any*/),
                        "storageKey": null
                      },
                      (v15/*:: as any*/),
                      (v16/*:: as any*/)
                    ],
                    "storageKey": null
                  }
                ],
                "storageKey": null
              },
              (v17/*:: as any*/)
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

(node as any).hash = "8a7fb4b145b13d315377bc81904d6bc8";

export default node;
