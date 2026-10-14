/**
 * @generated SignedSource<<c30bfb8758524113be819c4358d49e57>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityEditMutation$variables = {
  categoryId: string;
  comment?: string | null | undefined;
  endTime: number;
  id: string;
  startTime: number;
};
export type ActivityEditMutation$data = {
  readonly updatePeriodTimeCategory: {
    readonly category: {
      readonly id: string;
      readonly name: string;
    } | null | undefined;
    readonly comment: string | null | undefined;
    readonly endTime: number | null | undefined;
    readonly id: string;
    readonly startTime: number;
  };
};
export type ActivityEditMutation = {
  response: ActivityEditMutation$data;
  variables: ActivityEditMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "categoryId"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "comment"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "endTime"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v6 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "categoryId",
        "variableName": "categoryId"
      },
      {
        "kind": "Variable",
        "name": "comment",
        "variableName": "comment"
      },
      {
        "kind": "Variable",
        "name": "endTime",
        "variableName": "endTime"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "startTime",
        "variableName": "startTime"
      }
    ],
    "concreteType": "Period",
    "kind": "LinkedField",
    "name": "updatePeriodTimeCategory",
    "plural": false,
    "selections": [
      (v5/*:: as any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "startTime",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "endTime",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "comment",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Category",
        "kind": "LinkedField",
        "name": "category",
        "plural": false,
        "selections": [
          (v5/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "name",
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
      (v2/*:: as any*/),
      (v3/*:: as any*/),
      (v4/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityEditMutation",
    "selections": (v6/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v3/*:: as any*/),
      (v4/*:: as any*/),
      (v2/*:: as any*/),
      (v0/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "ActivityEditMutation",
    "selections": (v6/*:: as any*/)
  },
  "params": {
    "cacheID": "95867046619766d85abb34222f76f8b3",
    "id": null,
    "metadata": {},
    "name": "ActivityEditMutation",
    "operationKind": "mutation",
    "text": "mutation ActivityEditMutation(\n  $id: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $categoryId: ID!\n  $comment: String\n) {\n  updatePeriodTimeCategory(id: $id, startTime: $startTime, endTime: $endTime, categoryId: $categoryId, comment: $comment) {\n    id\n    startTime\n    endTime\n    comment\n    category {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "60325151a5305515c9c2a345508404cb";

export default node;
