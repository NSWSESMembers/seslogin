/**
 * @generated SignedSource<<ef92158d8975dee192d1013f5d26b80c>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ActivityCurrentQuery$variables = {
  after?: string | null | undefined;
  first: number;
  location: string;
};
export type ActivityCurrentQuery$data = {
  readonly location: {
    readonly id: string;
    readonly periods: {
      readonly edges: ReadonlyArray<{
        readonly node: {
          readonly " $fragmentSpreads": FragmentRefs<"ActivityCurrent_periodName" | "ActivityListTable_period">;
        };
      }>;
      readonly pageInfo: {
        readonly endCursor: string | null | undefined;
        readonly hasNextPage: boolean;
      };
    };
  };
};
export type ActivityCurrentQuery = {
  response: ActivityCurrentQuery$data;
  variables: ActivityCurrentQuery$variables;
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
  "name": "first"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "location"
},
v3 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "location"
  }
],
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v5 = [
  {
    "kind": "Variable",
    "name": "after",
    "variableName": "after"
  },
  {
    "kind": "Variable",
    "name": "first",
    "variableName": "first"
  },
  {
    "kind": "Literal",
    "name": "onlyActive",
    "value": true
  }
],
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "startTime",
  "storageKey": null
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "endTime",
  "storageKey": null
},
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "nitcExportStatus",
  "storageKey": null
},
v9 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "nitcEventId",
  "storageKey": null
},
v10 = {
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
v11 = {
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
v12 = {
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
      (v2/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityCurrentQuery",
    "selections": [
      {
        "alias": null,
        "args": (v3/*: any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v4/*: any*/),
          {
            "alias": null,
            "args": (v5/*: any*/),
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
                          (v4/*: any*/),
                          (v6/*: any*/),
                          (v7/*: any*/),
                          (v8/*: any*/),
                          (v9/*: any*/),
                          (v10/*: any*/)
                        ],
                        "args": null,
                        "argumentDefinitions": []
                      },
                      {
                        "kind": "InlineDataFragmentSpread",
                        "name": "ActivityCurrent_periodName",
                        "selections": [
                          (v11/*: any*/)
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
              (v12/*: any*/)
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
      (v2/*: any*/),
      (v1/*: any*/),
      (v0/*: any*/)
    ],
    "kind": "Operation",
    "name": "ActivityCurrentQuery",
    "selections": [
      {
        "alias": null,
        "args": (v3/*: any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v4/*: any*/),
          {
            "alias": null,
            "args": (v5/*: any*/),
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
                      (v4/*: any*/),
                      (v6/*: any*/),
                      (v7/*: any*/),
                      (v8/*: any*/),
                      (v9/*: any*/),
                      (v10/*: any*/),
                      (v11/*: any*/)
                    ],
                    "storageKey": null
                  }
                ],
                "storageKey": null
              },
              (v12/*: any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "5e5ea6912c612b2928d6b75c6a447b3e",
    "id": null,
    "metadata": {},
    "name": "ActivityCurrentQuery",
    "operationKind": "query",
    "text": "query ActivityCurrentQuery(\n  $location: ID!\n  $first: Int!\n  $after: String\n) {\n  location(id: $location) {\n    id\n    periods(onlyActive: true, first: $first, after: $after) {\n      edges {\n        node {\n          ...ActivityListTable_period\n          ...ActivityCurrent_periodName\n          id\n        }\n      }\n      pageInfo {\n        hasNextPage\n        endCursor\n      }\n    }\n  }\n}\n\nfragment ActivityCurrent_periodName on Period {\n  person {\n    id\n    firstName\n    lastName\n  }\n}\n\nfragment ActivityListTable_period on Period {\n  id\n  startTime\n  endTime\n  nitcExportStatus\n  nitcEventId\n  category {\n    id\n    name\n  }\n}\n"
  }
};
})();

(node as any).hash = "a5a3e789bf19339fc4db820647a9d44a";

export default node;
