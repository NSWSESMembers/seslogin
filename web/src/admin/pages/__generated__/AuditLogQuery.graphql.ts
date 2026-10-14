/**
 * @generated SignedSource<<629f0dbca12dfb2a377507f5cef05d6c>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type AuditEntityType = "API_TOKEN" | "CATEGORY" | "LOCATION" | "NITC_GROUP" | "NITC_TAG" | "OAUTH_GRANT" | "PERIOD" | "PERSON" | "SESSION" | "USER" | "USER_TOKEN" | "WEBAUTHN_CREDENTIAL" | "%future added value";
export type AuditLogQuery$variables = {
  after?: string | null | undefined;
  all: boolean;
  entityType?: AuditEntityType | null | undefined;
  first: number;
  location: string;
};
export type AuditLogQuery$data = {
  readonly auditLog?: {
    readonly edges: ReadonlyArray<{
      readonly node: {
        readonly " $fragmentSpreads": FragmentRefs<"AuditLog_entry">;
      };
    }>;
    readonly pageInfo: {
      readonly endCursor: string | null | undefined;
      readonly hasNextPage: boolean;
    };
  };
  readonly location?: {
    readonly auditLog: {
      readonly edges: ReadonlyArray<{
        readonly node: {
          readonly " $fragmentSpreads": FragmentRefs<"AuditLog_entry">;
        };
      }>;
      readonly pageInfo: {
        readonly endCursor: string | null | undefined;
        readonly hasNextPage: boolean;
      };
    };
    readonly id: string;
  };
};
export type AuditLogQuery = {
  response: AuditLogQuery$data;
  variables: AuditLogQuery$variables;
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
  "name": "all"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "entityType"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "first"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "location"
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
    "name": "entityType",
    "variableName": "entityType"
  },
  {
    "kind": "Variable",
    "name": "first",
    "variableName": "first"
  }
],
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "timestamp",
  "storageKey": null
},
v9 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "action",
  "storageKey": null
},
v10 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "entityType",
  "storageKey": null
},
v11 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "entityId",
  "storageKey": null
},
v12 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "entityLabel",
  "storageKey": null
},
v13 = {
  "alias": null,
  "args": null,
  "concreteType": "Location",
  "kind": "LinkedField",
  "name": "location",
  "plural": false,
  "selections": [
    (v6/*:: as any*/),
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
v14 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "kind",
  "storageKey": null
},
v15 = {
  "alias": "actorId",
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v16 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "via",
  "storageKey": null
},
v17 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "label",
  "storageKey": null
},
v18 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "ip",
  "storageKey": null
},
v19 = {
  "alias": null,
  "args": null,
  "concreteType": "AuditFieldChange",
  "kind": "LinkedField",
  "name": "changes",
  "plural": true,
  "selections": [
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "field",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "before",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "after",
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
},
v21 = {
  "alias": null,
  "args": (v7/*:: as any*/),
  "concreteType": "AuditEntryConnection",
  "kind": "LinkedField",
  "name": "auditLog",
  "plural": false,
  "selections": [
    {
      "alias": null,
      "args": null,
      "concreteType": "AuditEntryEdge",
      "kind": "LinkedField",
      "name": "edges",
      "plural": true,
      "selections": [
        {
          "alias": null,
          "args": null,
          "concreteType": "AuditEntry",
          "kind": "LinkedField",
          "name": "node",
          "plural": false,
          "selections": [
            {
              "kind": "InlineDataFragmentSpread",
              "name": "AuditLog_entry",
              "selections": [
                (v6/*:: as any*/),
                (v8/*:: as any*/),
                (v9/*:: as any*/),
                (v10/*:: as any*/),
                (v11/*:: as any*/),
                (v12/*:: as any*/),
                {
                  "kind": "CatchField",
                  "field": (v13/*:: as any*/),
                  "to": "RESULT"
                },
                {
                  "alias": null,
                  "args": null,
                  "concreteType": "AuditActor",
                  "kind": "LinkedField",
                  "name": "actor",
                  "plural": false,
                  "selections": [
                    (v14/*:: as any*/),
                    (v15/*:: as any*/),
                    (v16/*:: as any*/),
                    {
                      "kind": "CatchField",
                      "field": (v17/*:: as any*/),
                      "to": "RESULT"
                    }
                  ],
                  "storageKey": null
                },
                (v18/*:: as any*/),
                (v19/*:: as any*/)
              ],
              "args": null,
              "argumentDefinitions": ([]/*:: as any*/)
            }
          ],
          "storageKey": null
        }
      ],
      "storageKey": null
    },
    (v20/*:: as any*/)
  ],
  "storageKey": null
},
v22 = {
  "alias": null,
  "args": (v7/*:: as any*/),
  "concreteType": "AuditEntryConnection",
  "kind": "LinkedField",
  "name": "auditLog",
  "plural": false,
  "selections": [
    {
      "alias": null,
      "args": null,
      "concreteType": "AuditEntryEdge",
      "kind": "LinkedField",
      "name": "edges",
      "plural": true,
      "selections": [
        {
          "alias": null,
          "args": null,
          "concreteType": "AuditEntry",
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
            (v13/*:: as any*/),
            {
              "alias": null,
              "args": null,
              "concreteType": "AuditActor",
              "kind": "LinkedField",
              "name": "actor",
              "plural": false,
              "selections": [
                (v14/*:: as any*/),
                (v15/*:: as any*/),
                (v16/*:: as any*/),
                (v17/*:: as any*/)
              ],
              "storageKey": null
            },
            (v18/*:: as any*/),
            (v19/*:: as any*/)
          ],
          "storageKey": null
        }
      ],
      "storageKey": null
    },
    (v20/*:: as any*/)
  ],
  "storageKey": null
};
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
    "name": "AuditLogQuery",
    "selections": [
      {
        "condition": "all",
        "kind": "Condition",
        "passingValue": false,
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
              (v21/*:: as any*/)
            ],
            "storageKey": null
          }
        ]
      },
      {
        "condition": "all",
        "kind": "Condition",
        "passingValue": true,
        "selections": [
          (v21/*:: as any*/)
        ]
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v4/*:: as any*/),
      (v1/*:: as any*/),
      (v3/*:: as any*/),
      (v0/*:: as any*/),
      (v2/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "AuditLogQuery",
    "selections": [
      {
        "condition": "all",
        "kind": "Condition",
        "passingValue": false,
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
              (v22/*:: as any*/)
            ],
            "storageKey": null
          }
        ]
      },
      {
        "condition": "all",
        "kind": "Condition",
        "passingValue": true,
        "selections": [
          (v22/*:: as any*/)
        ]
      }
    ]
  },
  "params": {
    "cacheID": "4237c100a2af799ec2fd11bfad4f37c7",
    "id": null,
    "metadata": {},
    "name": "AuditLogQuery",
    "operationKind": "query",
    "text": "query AuditLogQuery(\n  $location: ID!\n  $all: Boolean!\n  $first: Int!\n  $after: String\n  $entityType: AuditEntityType\n) {\n  location(id: $location) @skip(if: $all) {\n    id\n    auditLog(first: $first, after: $after, entityType: $entityType) {\n      edges {\n        node {\n          ...AuditLog_entry\n          id\n        }\n      }\n      pageInfo {\n        hasNextPage\n        endCursor\n      }\n    }\n  }\n  auditLog(first: $first, after: $after, entityType: $entityType) @include(if: $all) {\n    edges {\n      node {\n        ...AuditLog_entry\n        id\n      }\n    }\n    pageInfo {\n      hasNextPage\n      endCursor\n    }\n  }\n}\n\nfragment AuditLog_entry on AuditEntry {\n  id\n  timestamp\n  action\n  entityType\n  entityId\n  entityLabel\n  location {\n    id\n    name\n  }\n  actor {\n    kind\n    actorId: id\n    via\n    label\n  }\n  ip\n  changes {\n    field\n    before\n    after\n  }\n}\n"
  }
};
})();

(node as any).hash = "f3ce65ff7ce56b753fc1427940a3ea77";

export default node;
