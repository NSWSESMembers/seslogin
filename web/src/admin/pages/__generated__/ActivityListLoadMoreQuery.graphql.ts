/**
 * @generated SignedSource<<0e39d0189c4cf4cae31890be688e0374>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ActivityListLoadMoreQuery$variables = {
  after?: string | null | undefined;
  categories?: ReadonlyArray<string> | null | undefined;
  first: number;
  location: string;
};
export type ActivityListLoadMoreQuery$data = {
  readonly location: {
    readonly id: string;
    readonly periods: {
      readonly edges: ReadonlyArray<{
        readonly node: {
          readonly " $fragmentSpreads": FragmentRefs<"ActivityListTable_period" | "ActivityList_periodName">;
        };
      }>;
      readonly pageInfo: {
        readonly endCursor: string | null | undefined;
        readonly hasNextPage: boolean;
      };
    };
  };
};
export type ActivityListLoadMoreQuery = {
  response: ActivityListLoadMoreQuery$data;
  variables: ActivityListLoadMoreQuery$variables;
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
  "name": "categories"
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
v4 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "location"
  }
],
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v6 = [
  {
    "kind": "Variable",
    "name": "after",
    "variableName": "after"
  },
  {
    "kind": "Variable",
    "name": "categories",
    "variableName": "categories"
  },
  {
    "kind": "Variable",
    "name": "first",
    "variableName": "first"
  }
],
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "personId",
  "storageKey": null
},
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "startTime",
  "storageKey": null
},
v9 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "endTime",
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
  "name": "nitcExportStatus",
  "storageKey": null
},
v12 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "nitcEventId",
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
  (v5/*: any*/),
  (v13/*: any*/)
],
v15 = {
  "alias": null,
  "args": null,
  "concreteType": "Session",
  "kind": "LinkedField",
  "name": "signedInSession",
  "plural": false,
  "selections": (v14/*: any*/),
  "storageKey": null
},
v16 = {
  "alias": null,
  "args": null,
  "concreteType": "Session",
  "kind": "LinkedField",
  "name": "signedOutSession",
  "plural": false,
  "selections": (v14/*: any*/),
  "storageKey": null
},
v17 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "category",
  "plural": false,
  "selections": [
    (v5/*: any*/),
    (v13/*: any*/),
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
v18 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "guestName",
  "storageKey": null
},
v19 = {
  "alias": null,
  "args": null,
  "concreteType": "Person",
  "kind": "LinkedField",
  "name": "person",
  "plural": false,
  "selections": [
    (v5/*: any*/),
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
    },
    {
      "alias": null,
      "args": null,
      "concreteType": "Location",
      "kind": "LinkedField",
      "name": "location",
      "plural": false,
      "selections": (v14/*: any*/),
      "storageKey": null
    }
  ],
  "storageKey": null
},
v20 = {
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
};
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
    "name": "ActivityListLoadMoreQuery",
    "selections": [
      {
        "alias": null,
        "args": (v4/*: any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v5/*: any*/),
          {
            "alias": null,
            "args": (v6/*: any*/),
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
                      {
                        "kind": "InlineDataFragmentSpread",
                        "name": "ActivityListTable_period",
                        "selections": [
                          (v5/*: any*/),
                          (v7/*: any*/),
                          (v8/*: any*/),
                          (v9/*: any*/),
                          (v10/*: any*/),
                          (v11/*: any*/),
                          (v12/*: any*/),
                          {
                            "kind": "CatchField",
                            "field": (v15/*: any*/),
                            "to": "RESULT"
                          },
                          {
                            "kind": "CatchField",
                            "field": (v16/*: any*/),
                            "to": "RESULT"
                          },
                          {
                            "kind": "CatchField",
                            "field": (v17/*: any*/),
                            "to": "RESULT"
                          }
                        ],
                        "args": null,
                        "argumentDefinitions": []
                      },
                      {
                        "kind": "InlineDataFragmentSpread",
                        "name": "ActivityList_periodName",
                        "selections": [
                          (v18/*: any*/),
                          {
                            "kind": "CatchField",
                            "field": (v19/*: any*/),
                            "to": "RESULT"
                          }
                        ],
                        "args": null,
                        "argumentDefinitions": []
                      }
                    ],
                    "storageKey": null
                  }
                ],
                "storageKey": null
              },
              (v20/*: any*/)
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
      (v1/*: any*/)
    ],
    "kind": "Operation",
    "name": "ActivityListLoadMoreQuery",
    "selections": [
      {
        "alias": null,
        "args": (v4/*: any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v5/*: any*/),
          {
            "alias": null,
            "args": (v6/*: any*/),
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
                      (v5/*: any*/),
                      (v7/*: any*/),
                      (v8/*: any*/),
                      (v9/*: any*/),
                      (v10/*: any*/),
                      (v11/*: any*/),
                      (v12/*: any*/),
                      (v15/*: any*/),
                      (v16/*: any*/),
                      (v17/*: any*/),
                      (v18/*: any*/),
                      (v19/*: any*/)
                    ],
                    "storageKey": null
                  }
                ],
                "storageKey": null
              },
              (v20/*: any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "fb650927bd5e3a054040ebff43b20771",
    "id": null,
    "metadata": {},
    "name": "ActivityListLoadMoreQuery",
    "operationKind": "query",
    "text": "query ActivityListLoadMoreQuery(\n  $location: ID!\n  $first: Int!\n  $after: String\n  $categories: [ID!]\n) {\n  location(id: $location) {\n    id\n    periods(first: $first, after: $after, categories: $categories) {\n      edges {\n        node {\n          ...ActivityListTable_period\n          ...ActivityList_periodName\n          id\n        }\n      }\n      pageInfo {\n        hasNextPage\n        endCursor\n      }\n    }\n  }\n}\n\nfragment ActivityListTable_period on Period {\n  id\n  personId\n  startTime\n  endTime\n  comment\n  nitcExportStatus\n  nitcEventId\n  signedInSession {\n    id\n    name\n  }\n  signedOutSession {\n    id\n    name\n  }\n  category {\n    id\n    name\n    isVirtual\n  }\n}\n\nfragment ActivityList_periodName on Period {\n  guestName\n  person {\n    id\n    firstName\n    lastName\n    memberNumber\n    location {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "12567a1983feeec7ec380c07e6fe3999";

export default node;
