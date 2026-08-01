/**
 * @generated SignedSource<<1fd41a5301b3980fa4b5021d6cf87b87>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type PeriodEditFormMutation$variables = {
  categoryId: string;
  endTime: number;
  id: string;
  startTime: number;
};
export type PeriodEditFormMutation$data = {
  readonly updatePeriodTimeCategory: {
    readonly category: {
      readonly id: string;
      readonly name: string;
    } | null | undefined;
    readonly endTime: number | null | undefined;
    readonly id: string;
    readonly startTime: number;
  };
};
export type PeriodEditFormMutation = {
  response: PeriodEditFormMutation$data;
  variables: PeriodEditFormMutation$variables;
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
  "name": "endTime"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
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
v5 = [
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
      (v4/*: any*/),
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
    "name": "PeriodEditFormMutation",
    "selections": (v5/*: any*/),
    "type": "MutationRoot",
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
    "name": "PeriodEditFormMutation",
    "selections": (v5/*: any*/)
  },
  "params": {
    "cacheID": "8b2217aeb0e02a5643958e24cb239c1b",
    "id": null,
    "metadata": {},
    "name": "PeriodEditFormMutation",
    "operationKind": "mutation",
    "text": "mutation PeriodEditFormMutation(\n  $id: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $categoryId: ID!\n) {\n  updatePeriodTimeCategory(id: $id, startTime: $startTime, endTime: $endTime, categoryId: $categoryId) {\n    id\n    startTime\n    endTime\n    category {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "354bad3a6788f0b06173f7aa1d7a4470";

export default node;
